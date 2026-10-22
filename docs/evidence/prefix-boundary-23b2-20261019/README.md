# Required complete raw audit prefix

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The actual full HIR integration harness reproduced its hosted failure before
this change: the current raw prefix omitted the JSON probe immediately after
its route-debits parent, although the existing complete-prefix assertion
requires that child there. Both raw inventories included it later instead.

Move that same include into a concat with its original parent. The original
32-piece prefix, nonempty tail, uniqueness and hostile assertions remain byte
identical. The closed actual macro grammar proves all 1421 HIR pieces and all
783 owned-projection pieces retain their multiplicities, with every other
piece in exactly its prior relative order. The broad 1020-piece capacity-table
inventory is a different namespace and is unchanged.

Normal compiler/runtime implementation, physical limits, reserved floors and
work limits do not change. Complete finite source views and both actual
source-role domains are rebound with the unchanged original hostile controls.
Source metadata sealing is one-shot and grants no executable qualification.
The complete original HIR harness is rebuilt against that sealed metadata and
must pass before this batch is committed; its separate fixed receipt records
the actual result. This is not a full quality-profile or hosted-CI claim.
