"""Candidate phase monitor. Fresh driver adoption requires root review."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import subprocess
import time

HELPER_PATH = Path(__file__).with_name("semaprax_pressure_observation_v3.py")
HELPER_SHA = "5f1d756491ccc5dd662afe6e7ed744b7e65e8e0a45bc7e420b358ec531cec90b"
if hashlib.sha256(HELPER_PATH.read_bytes()).hexdigest() != HELPER_SHA:
    raise RuntimeError("frozen observation helper drift")
spec = importlib.util.spec_from_file_location("pressure_observation", HELPER_PATH)
observation = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observation)


def provenance():
    return dict(observation_helper_path=str(HELPER_PATH), observation_helper_sha256=HELPER_SHA,
                phase_monitor_path=str(Path(__file__).resolve()),
                phase_monitor_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())


def process_rows(text, pgid):
    members = []
    for row in text.splitlines():
        values = row.split(None, 4)
        if len(values) != 5 or not all(value.isdecimal() for value in values[:4]):
            raise observation.ObservationError("process RSS observation unavailable")
        pid, parent, group, rss = map(int, values[:4])
        if pid <= 0:
            raise observation.ObservationError("invalid process identity")
        if group == pgid:
            members.append(dict(pid=pid, ppid=parent, rss_bytes=rss * 1024, name=values[4]))
    return members


def free_percent(text):
    found = re.search(r"System-wide memory free percentage: (\d+)%", text)
    if not found or not 0 <= int(found[1]) <= 100:
        raise observation.ObservationError("system free observation unavailable")
    return int(found[1])


def run_phase(command, *, cwd, env, stdout, stderr, rss_limit_bytes, emit,
              command_index=0, wall_limit=None, wall_deadline_inclusive=False, disk_free=None, bind=None,
              run=subprocess.run, popen=subprocess.Popen, clock=time.monotonic,
              sleep=time.sleep, killpg=os.killpg, getpgrp=os.getpgrp,
              executor_factory=ThreadPoolExecutor):
    """Preserves guards; separate temporary worker keeps telemetry off guard path.

    At most one telemetry task, with five-second command/sample deadline. No
    waiting for it before own-group settlement. Runner timeouts are mandatory.
    """
    start = clock()
    reason, errors, samples, peaks = None, [], [], {}
    failed_telemetry_samples = []
    baseline, post_stop, settlement = None, None, None
    child, executor, future = None, None, None
    peak, last_free, last_telemetry, interrupt_code = 0, None, start, None
    admission_free = None
    max_swap_growth = 0
    kernel_version = os.uname().version
    post_stop_status = "not_requested"

    def refuse(message):
        nonlocal reason
        reason = observation.first_refusal(reason, message)

    def collect(sample, label):
        nonlocal max_swap_growth
        row = dict(command_index=command_index, label=label,
                   elapsed_seconds=clock() - start, telemetry=sample)
        if baseline is not None:
            row["swap_used_delta_bytes"] = observation.swap_delta(baseline["swap"], sample["swap"])
            try:
                row["vm_counter_deltas"] = observation.vm_deltas(baseline["vm"], sample["vm"])
            except observation.ObservationError as error:
                row["vm_delta_status"] = "invalid_baseline"
                row["vm_delta_error"] = str(error)
                failed_telemetry_samples.append(row)
                emit(row)
                raise
            max_swap_growth = max(max_swap_growth, row["swap_used_delta_bytes"])
        emit(row)
        samples.append(row)

    try:
        baseline = observation.telemetry_sample(clock() + 5, run, clock, kernel_version)
        collect(baseline, "baseline")
        admission_free = free_percent(observation.bounded_command(
            ["/usr/bin/memory_pressure", "-Q"], clock()+5, run, clock))
        if admission_free < 30:
            raise observation.ObservationError("fresh child admission requires free >=30percent")
        executor = executor_factory(max_workers=1)
        child = popen(command, cwd=cwd, env=env, stdout=stdout, stderr=stderr,
                      start_new_session=True)
        while child.poll() is None:
            deadline = clock() + 5
            members = process_rows(observation.bounded_command(
                ["/bin/ps", "-axo", "pid=,ppid=,pgid=,rss=,comm="], deadline, run, clock, max_bytes=131072), child.pid)
            rss = sum(member["rss_bytes"] for member in members)
            peak = max(peak, rss)
            for member in members:
                previous = peaks.get(member["pid"])
                if previous is None or previous["rss_bytes"] < member["rss_bytes"]:
                    peaks[member["pid"]] = member
            if rss > rss_limit_bytes:
                refuse("process-group RSS cap")
            # Guard first: never await telemetry after a selected refusal.
            if reason is None:
                last_free = free_percent(observation.bounded_command(
                    ["/usr/bin/memory_pressure", "-Q"], clock() + 5, run, clock))
                if last_free < 15:
                    refuse("system memory free below15percent")
            if wall_limit is not None and (clock() - start >= wall_limit if wall_deadline_inclusive else clock() - start > wall_limit):
                refuse("phase time cap")
            if disk_free is not None and disk_free() < 5 * 1024**3:
                refuse("5GiB disk floor")
            emit(dict(command_index=command_index, label="guard", elapsed_seconds=clock()-start,
                      group_rss_bytes=rss, system_free_percent=last_free, processes=members))
            if reason is not None:
                break
            if future is not None and future.done():
                collect(future.result(), "monitor")
                future = None
            if future is None and clock() - last_telemetry >= 2:
                future = executor.submit(observation.telemetry_sample, clock() + 5, run, clock, kernel_version)
                last_telemetry = clock()
            sleep(1)
    except BaseException as error:
        errors.append(type(error).__name__ + ": " + str(error))
        refuse("memory watchdog observation failed: " + type(error).__name__)
        if isinstance(error, SystemExit):
            interrupt_code = error.code if isinstance(error.code, int) and error.code else 1
        elif isinstance(error, KeyboardInterrupt):
            interrupt_code = 130
    finally:
        if child is not None:
            def read_members(pgid, deadline):
                return observation.parse_group_members(observation.bounded_command(
                    ["/bin/ps", "-axo", "pid=,pgid="], deadline, run, clock, max_bytes=131072), pgid)
            try:
                settlement = observation.settle_owned_group(
                    leader_pid=child.pid, pgid=child.pid, watchdog_pgid=getpgrp(),
                    deadline=clock()+8, grace_seconds=3, read_members=read_members,
                    send_signal=killpg, clock=clock, sleep=sleep, refusal=reason)
                reason = observation.first_refusal(reason, settlement["resource_refusal"])
                if settlement["signals"] and reason is None:
                    refuse("owned group remained after leader exit")
            except BaseException as error:
                errors.append("settlement: " + type(error).__name__ + ": " + str(error))
                refuse("owned group settlement failed")
                settlement = dict(owned_pgid=child.pid, confirmed_empty=False,
                                  observation_errors=[type(error).__name__ + ": " + str(error)],
                                  resource_refusal=reason, signals=[], finished=clock())
                # Last-resort exact owned-group KILL; never an unrelated group.
                try:
                    if child.pid > 1 and child.pid != getpgrp():
                        killpg(child.pid, 9)
                        settlement["signals"].append(9)
                except BaseException as cleanup_error:
                    errors.append("emergency kill: " + type(cleanup_error).__name__)
            try:
                code = child.wait(timeout=3)
            except BaseException as error:
                errors.append("bounded reap: " + type(error).__name__)
                refuse("direct child bounded reap failed")
                code = 1
        else:
            code = 125
        # Settlement attempt precedes telemetry waits; only explicit empty
        # confirmation permits the post-stop host sample.
        if future is not None:
            try:
                collect(future.result(timeout=5), "monitor_completed_after_settlement")
            except BaseException as error:
                errors.append("pending telemetry: " + type(error).__name__)
                refuse("memory watchdog observation failed: " + type(error).__name__)
        if executor is not None:
            try:
                executor.shutdown(wait=False, cancel_futures=True)
            except BaseException as error:
                errors.append("observer shutdown: " + type(error).__name__)
                refuse("observer shutdown failed")
        if child is not None and settlement is not None and settlement["confirmed_empty"]:
            try:
                post_stop = observation.telemetry_sample(clock()+5, run, clock, kernel_version)
                collect(post_stop, "post_stop_after_settlement")
                post_stop_status = "collected_after_confirmed_empty_group"
            except BaseException as error:
                errors.append("post-stop telemetry: " + type(error).__name__)
                refuse("post-stop observation failed")
                post_stop_status = "failed_after_confirmed_empty_group"
        elif child is not None:
            post_stop_status = "skipped_unconfirmed_group_settlement"
        if bind is not None:
            try:
                bind()
                binding = "unchanged"
            except BaseException as error:
                binding = "failed"
                errors.append("source binding: " + type(error).__name__ + ": " + str(error))
                refuse("source/manifest drift")
        else:
            binding = "not_requested"
    if interrupt_code is not None:
        code = interrupt_code
    if reason is not None and code == 0:
        code = 1
    return dict(command=command, executed=child is not None, exit_code=code,
                resource_refusal=reason, observation_errors=errors,
                peak_process_group_rss_bytes=peak, rss_limit_bytes=rss_limit_bytes,
                last_system_free_percent=last_free, process_high_water_marks=list(peaks.values()),
                wall_seconds=clock()-start, baseline_telemetry=baseline, post_stop_telemetry=post_stop,
                post_stop_observation_status=post_stop_status,
                admission_system_free_percent=admission_free,
                telemetry_sample_count=len(samples), maximum_positive_swap_used_delta_bytes=max_swap_growth,
                failed_telemetry_samples=failed_telemetry_samples,
                group_settlement=settlement, source_binding_status=binding, **provenance())
