# Test-media fixtures

`./tests/fixtures/generate.sh` generates 20-second synthetic H.264 and HEVC video files with identical underlying moving test patterns and an AAC audio track. Install FFmpeg/ffprobe first. Use only self-created or properly licensed media; no movies are bundled.

The outputs under `tests/fixtures/generated/` are ignored. This is a lightweight fixture to exercise codec switching; it is **not** evidence that bitstream-level mixing or content alignment currently works.

Variations for future experiments: add known edit offsets, different framerates, dropped frames, corrupt pieces, network shaping and two distinct torrents containing the same bytes. Record exact ffmpeg version and checksums with every benchmark.
