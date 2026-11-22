# Evidence: controlled GitHub RED/GREEN + pending media decode gate

## Empirical CI-dispatch selection test (NOT GitLab Runner compute)

1. Baseline `f3a54af3adca5c600962816b37af24a08b143b6f`: external GitHub Actions callback `success`; actual individual job logs inaccessible.
2. RED `9e684f3139fc5112fb8d9670c24f1c22edfc1526`: inserted **only** `echo ALCHEMY_REQUIRED_HLS_JOB_PROBE_EXECUTED; exit 93` at the very top of `tests/integration/hls.sh`. GitLab API pipeline [#3098476918](https://gitlab.com/blond11qq/my_streamio/-/pipelines/3098476918) invoked [GitHub run #44964664052](https://github.com/giinii06/my_streamio-ci-mirror/actions/runs/44964664052) and received `github-actions/ci-hub-request=failed` at 2026-11-22 00:04:27 UTC.
3. GREEN `e7aa9679dce206a03e979703d2b4734afe738c79`: removed exactly those three probe lines and restored original shell script. [GitLab request #3098487129](https://gitlab.com/blond11qq/my_streamio/-/pipelines/3098487129) → [GitHub run #44964987246](https://github.com/giinii06/my_streamio-ci-mirror/actions/runs/44964987246), external context `success` at 2026-11-22 00:08:31 UTC.
4. GitLab `compare_refs(f3a54af3..e7aa9679, straight=true)` returned **zero file differences**; the source tree was restored exactly.

**Interpretation:** a controlled fault in the HLS script caused provider failure and its removal restored provider success, strong indirect evidence that this GitHub pipeline selects `tests/integration/hls.sh`. In that script, `--list | grep` forces the Rust `network_e2e::two_real_swarms_deliver_piece_verified_http_seek_bytes` test to be present, and a subsequent `cargo test --features network-fixture` invokes it. This supports the test-path inference, but independently authenticated job logs/check-out SHA are still absent.

## New acceptance extension (current candidate to be verified)

After HTTP Range reconstruction, write exact received bytes to isolated temp storage, FFprobe must report `h264` for primary and `hevc` for alternate, and FFmpeg must fully decode *all* streams with `-xerror` (including audio), preventing false success from transfer of undecryptable/unplayable bytes.

## Classification

- `dispatch`: observed success for RED/GREEN exact source SHA.
- `provider_result`: observed failed for RED and success for GREEN.
- `jobs_verified`: **unknown without GitHub individual job logs**; do NOT convert callback into direct proof.
- `e2e_verified`: **indirect inference for original peer-to-HTTP script; NOT yet executed for FFmpeg decode extension**.
- `security`: self-review only; test sessions bind loopback, disable DHT/trackers/LSD/UPnP; production credential/permissions unchanged.
- No GitLab Runner compute, no local Cargo executor, no Mac M5 evidence, no merge or deployment.
