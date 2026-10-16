"""OPT798 candidate helpers; no policy thresholds or automatic driver adoption.

All command execution and signalling are injected. SDK xsw_usage layout:
MacOSX.sdk/usr/include/sys/sysctl.h:539-544. VM counter semantics:
/usr/share/man/man1/vm_stat.1:37-83,115-119. Pressure labels require the exact
installed-kernel match and pinned XNU handler/constants provenance below.
"""
import ctypes
import hashlib
import re
import signal


class ObservationError(RuntimeError):
    pass


class SwapUsage(ctypes.Structure):
    _fields_ = [("total", ctypes.c_uint64), ("available", ctypes.c_uint64),
                ("used", ctypes.c_uint64), ("page_size", ctypes.c_uint32),
                ("encrypted", ctypes.c_int32)]


def decode_swapusage(raw):
    if not isinstance(raw, bytes) or len(raw) != ctypes.sizeof(SwapUsage):
        raise ObservationError("swapusage native structure size mismatch")
    value = SwapUsage.from_buffer_copy(raw)
    if value.used > value.total or value.available > value.total:
        raise ObservationError("swapusage exceeds total")
    if value.used + value.available != value.total or value.page_size <= 0:
        raise ObservationError("swapusage inconsistent totals/page size")
    if value.encrypted not in (0, 1):
        raise ObservationError("swapusage invalid boolean")
    return dict(total_bytes=value.total, available_bytes=value.available,
                used_bytes=value.used, page_size=value.page_size,
                encrypted=bool(value.encrypted), raw_sha256=hashlib.sha256(raw).hexdigest())


SUPPORTED_KERNEL_VERSION = "Darwin Kernel Version 25.5.0: Mon Apr 27 20:41:06 PDT 2026; root:xnu-12377.121.6~2/RELEASE_ARM64_T6030"
PRESSURE_MAPPING = dict(
    source_tag="xnu-12377.121.6",
    conversion_source="https://raw.githubusercontent.com/apple-oss-distributions/xnu/xnu-12377.121.6/bsd/kern/kern_memorystatus_notify.c",
    conversion_spans="1651-1679;1754-1768",
    constants_source="https://raw.githubusercontent.com/apple-oss-distributions/xnu/xnu-12377.121.6/bsd/sys/event_private.h",
    constants_span="276-278")


def kernel_pressure(text, kernel_version=None):
    if not isinstance(text, str) or not re.fullmatch(r"\s*\d+\s*", text):
        raise ObservationError("kernel pressure integer unavailable")
    raw = int(text)
    known = kernel_version == SUPPORTED_KERNEL_VERSION
    label = {1: "normal", 2: "warning", 4: "critical"}.get(raw) if known else None
    return dict(raw=raw, label=label, label_status="exact_kernel_mapping" if label else "unknown_mapping",
                kernel_version=kernel_version, mapping_provenance=PRESSURE_MAPPING.copy() if known else None)


VM_FIELDS = ("Pages free", "Pages inactive", "Pages purgeable", "File-backed pages",
             "Pages occupied by compressor", "Swapins", "Swapouts")


def parse_vm_stat(text):
    page = re.search(r"page size of (\d+) bytes", text)
    if not page or int(page[1]) <= 0:
        raise ObservationError("VM page size unavailable")
    values = {}
    for line in text.splitlines():
        match = re.fullmatch(r'\s*"?([^":]+)"?:\s*(\d+)\.\s*', line)
        if match and match[1] in VM_FIELDS:
            if match[1] in values:
                raise ObservationError("duplicate VM counter")
            values[match[1]] = int(match[2])
    if set(values) != set(VM_FIELDS):
        raise ObservationError("required VM counters unavailable")
    return dict(page_size=int(page[1]), counters=values)


def vm_deltas(before, after):
    if before["page_size"] != after["page_size"]:
        raise ObservationError("VM page size changed; baseline invalid")
    delta = {}
    for name in ("Swapins", "Swapouts"):
        value = after["counters"][name] - before["counters"][name]
        if value < 0:
            raise ObservationError("VM counter reset; baseline invalid")
        delta[name] = value
    return delta


def swap_delta(before, after):
    # Occupancy may decrease while accumulated swap traffic increases.
    return after["used_bytes"] - before["used_bytes"]


def bounded_command(command, deadline, run, clock, *, binary=False, max_bytes=65536):
    remaining = deadline - clock()
    if remaining <= 0:
        raise ObservationError("sample deadline expired")
    try:
        result = run(command, timeout=remaining, capture_output=True, check=False)
    except Exception as error:
        raise ObservationError("command observation failed: " + type(error).__name__) from error
    if clock() > deadline or result.returncode != 0:
        raise ObservationError("command deadline/status failure")
    raw = result.stdout
    if not isinstance(raw, bytes) or len(raw) > max_bytes:
        raise ObservationError("command output type/size failure")
    if binary:
        return raw
    try:
        return raw.decode("utf-8", errors="strict")
    except UnicodeError as error:
        raise ObservationError("command output encoding failure") from error


def telemetry_sample(deadline, run, clock, kernel_version=None):
    """One bounded, non-atomic sample; caller retains existing guard cadence.

    Required failures raise and must reach driver's own-group finally cleanup.
    No cached values are returned. Command windows expose observation skew.
    """
    windows = []

    def read(command, binary=False):
        start = clock()
        value = bounded_command(command, deadline, run, clock, binary=binary)
        windows.append(dict(command=command, started=start, finished=clock()))
        return value

    pressure = kernel_pressure(read(["/usr/sbin/sysctl", "-n", "kern.memorystatus_vm_pressure_level"]), kernel_version)
    swap = decode_swapusage(read(["/usr/sbin/sysctl", "-b", "vm.swapusage"], True))
    vm = parse_vm_stat(read(["/usr/bin/vm_stat"]))
    return dict(kernel_pressure=pressure, swap=swap, vm=vm, windows=windows,
                finished=clock(), atomic=False)


def first_refusal(existing, additional):
    return existing if existing is not None else additional


def parse_group_members(text, pgid):
    members = []
    for line in text.splitlines():
        fields = line.split()
        if len(fields) != 2 or not all(field.isdecimal() for field in fields):
            raise ObservationError("process group membership unavailable")
        pid, group = map(int, fields)
        if pid <= 0 or group < 0:
            raise ObservationError("invalid process identity")
        if group == pgid:
            members.append(pid)
    return members


def settle_owned_group(*, leader_pid, pgid, watchdog_pgid, deadline, grace_seconds,
                       read_members, send_signal, clock, sleep, refusal=None):
    """Settle only a recorded start_new_session=True child group.

    read_members(pgid, deadline) must use bounded_command+parse_group_members;
    send_signal(pgid, signal) is injected killpg. Leader liveness is irrelevant.
    A membership failure never proves emptiness. All operations, including the
    caller's wait/reap, need bounds; receipt must retain confirmed_empty=False.
    """
    if pgid <= 1 or pgid != leader_pid or pgid == watchdog_pgid:
        raise ObservationError("not an isolated owned child PGID")
    errors, sent, failures = [], [], []

    def members():
        nonlocal refusal
        if clock() >= deadline:
            return None
        try:
            found = read_members(pgid, deadline)
            if clock() > deadline:
                raise ObservationError("owned membership observation exceeded deadline")
            if not isinstance(found, list) or any(not isinstance(pid, int) or pid <= 0 for pid in found):
                raise ObservationError("invalid owned membership result")
            return found
        except (Exception, KeyboardInterrupt) as error:
            errors.append(type(error).__name__ + ": " + str(error))
            failures.append(dict(operation="membership", error_type=type(error).__name__,
                                 errno=getattr(error, "errno", None)))
            refusal = first_refusal(refusal, "owned group observation failed")
            return None

    def send(kind):
        nonlocal refusal
        try:
            send_signal(pgid, kind)
            sent.append(int(kind))
        except ProcessLookupError:
            pass  # Race is harmless, but does not replace empty confirmation.
        except (Exception, KeyboardInterrupt) as error:
            errors.append(type(error).__name__ + ": " + str(error))
            failures.append(dict(operation="killpg", signal=int(kind), error_type=type(error).__name__,
                                 errno=getattr(error, "errno", None)))
            refusal = first_refusal(refusal, "owned group signalling failed")

    found = members()
    if found != []:
        send(signal.SIGTERM)
        grace_end = min(deadline, clock() + max(0, grace_seconds))
        interrupted = False
        while found != [] and clock() < grace_end:
            try:
                sleep(min(0.1, grace_end - clock()))
            except (Exception, KeyboardInterrupt) as error:
                errors.append(type(error).__name__ + ": " + str(error))
                refusal = first_refusal(refusal, "owned settlement wait interrupted")
                interrupted = True
                break
            found = members()
        if found != []:
            send(signal.SIGKILL)
            while found != [] and clock() < deadline and not interrupted:
                try:
                    sleep(min(0.1, deadline - clock()))
                except (Exception, KeyboardInterrupt) as error:
                    errors.append(type(error).__name__ + ": " + str(error))
                    refusal = first_refusal(refusal, "owned settlement wait interrupted")
                    interrupted = True
                    break
                found = members()
            if interrupted:
                found = members()  # One bounded confirmation after emergency KILL.
    confirmed = found == []
    if not confirmed:
        refusal = first_refusal(refusal, "owned group settlement unconfirmed")
    return dict(owned_pgid=pgid, confirmed_empty=confirmed, signals=sent,
                operation_failures=failures,
                observation_errors=errors, resource_refusal=refusal, finished=clock())
