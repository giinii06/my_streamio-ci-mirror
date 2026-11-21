# FFmpeg encoder probe regression

A real ARM64 GitLab `media_hls_verify` run on SHA
`5c985616a22855176f6f76cb25765b8467db297c`
failed with exit 141 immediately after:

```sh
ffmpeg -hide_banner -encoders 2>/dev/null | grep -q libx264
```

The command was executed under CI's pipefail behavior. When grep -q exits
as soon as it finds a matching early encoder, FFmpeg can still be writing the
rest of the encoder list, receive SIGPIPE and exit with 141. The check wrongly
fails even when the encoder exists.

For both CI providers we use `grep -E PATTERN >/dev/null` **without**
`-q`; grep consumes the complete output and avoids closing the pipe early.
The pattern matches the exact encoder token. The HLS job itself is the
end-to-end regression check: it must run through the encoder probes and then
execute `tests/integration/hls.sh`, verifying the actual output.

Do not hide failures by adding `|| true` or disabling pipefail.
