# Spike findings

## Machine B? (OpenVINO/NPU present) - Nep-Sagar - 2026-10-11

### Spike 0: catalog (SDK 2.1.0, Microsoft.AI.Foundry.Local.Runtime 2.1.0)

Notes:
- `nemotron-speech-streaming-en-0.6b` resolves. It has only a **CPU** variant (generic-cpu:3); no NPU/GPU variant.
- Also present: `nemotron-3.5-asr-streaming-0.6b` (CPU only), `parakeet-tdt-0.6b-v2` (CPU only).
- Chat models with NPU variants (openvino-npu): phi-4-mini, phi-3-mini-4k, qwen2.5-0.5b, deepseek-r1-7b, mistral-7b-v0.2.
- EPs registered: OpenVINO, WebGPU.
- SDK 2.x drift vs plan/docs: no `winml` feature (bundled); `info()` returns Result; native DLLs only fetched at build when FOUNDRY_LOCAL_RUNTIME_VERSION is set.

~~~text
== Execution providers ==
  OpenVINOExecutionProvider                registered=false
  WebGpuExecutionProvider                  registered=false
  success=true status=All requested execution providers were registered successfully. registered=[OpenVINOExecutionProvider, WebGpuExecutionProvider] failed=[]

== Catalog (alias -> variants) ==
  49 models
  deepseek-r1-1.5b
    - DeepSeek-R1-Distill-Qwen-1.5B-openvino-gpu:2            device=GPU  ep=OpenVINOExecutionProvider
  deepseek-r1-14b
    - DeepSeek-R1-Distill-Qwen-14B-openvino-gpu:2             device=GPU  ep=OpenVINOExecutionProvider
    - deepseek-r1-distill-qwen-14b-generic-gpu:4              device=GPU  ep=WebGPUExecutionProvider
    - deepseek-r1-distill-qwen-14b-generic-cpu:4              device=CPU  ep=CPUExecutionProvider
  deepseek-r1-7b
    - DeepSeek-R1-Distill-Qwen-7B-openvino-npu:4              device=NPU  ep=OpenVINOExecutionProvider
    - DeepSeek-R1-Distill-Qwen-7B-openvino-gpu:2              device=GPU  ep=OpenVINOExecutionProvider
    - deepseek-r1-distill-qwen-7b-generic-gpu:4               device=GPU  ep=WebGPUExecutionProvider
    - deepseek-r1-distill-qwen-7b-generic-cpu:4               device=CPU  ep=CPUExecutionProvider
  gemma-4-e2b-it
    - gemma-4-e2b-it-generic-gpu:3                            device=GPU  ep=WebGPUExecutionProvider
    - gemma-4-e2b-it-generic-cpu:3                            device=CPU  ep=CPUExecutionProvider
  gpt-oss-20b
    - gpt-oss-20b-generic-gpu:1                               device=GPU  ep=WebGPUExecutionProvider
    - gpt-oss-20b-generic-cpu:1                               device=CPU  ep=CPUExecutionProvider
  ministral-3-3b-instruct-2512
    - ministral-3-3b-instruct-2512-generic-gpu:2              device=GPU  ep=WebGPUExecutionProvider
    - ministral-3-3b-instruct-2512-generic-cpu:2              device=CPU  ep=CPUExecutionProvider
  mistral-7b-v0.2
    - Mistral-7B-Instruct-v0-2-openvino-npu:3                 device=NPU  ep=OpenVINOExecutionProvider
    - Mistral-7B-Instruct-v0-2-openvino-gpu:2                 device=GPU  ep=OpenVINOExecutionProvider
    - mistralai-Mistral-7B-Instruct-v0-2-generic-gpu:2        device=GPU  ep=WebGPUExecutionProvider
    - mistralai-Mistral-7B-Instruct-v0-2-generic-cpu:3        device=CPU  ep=CPUExecutionProvider
  mistral-nemo-12b-instruct
    - mistral-nemo-12b-instruct-generic-gpu:1                 device=GPU  ep=WebGPUExecutionProvider
    - mistral-nemo-12b-instruct-generic-cpu:1                 device=CPU  ep=CPUExecutionProvider
  nemotron-3.5-asr-streaming-0.6b
    - nemotron-3.5-asr-streaming-0.6b-generic-cpu:3           device=CPU  ep=CPUExecutionProvider
  nemotron-speech-streaming-en-0.6b
    - nemotron-speech-streaming-en-0.6b-generic-cpu:3         device=CPU  ep=CPUExecutionProvider
  nemotron-speech-streaming-es-0.6b
    - nemotron-speech-streaming-es-0.6b-ft-generic-cpu:1      device=CPU  ep=CPUExecutionProvider
  olmo-3-7b-instruct
    - olmo-3-7b-instruct-generic-gpu:1                        device=GPU  ep=WebGPUExecutionProvider
    - olmo-3-7b-instruct-generic-cpu:1                        device=CPU  ep=CPUExecutionProvider
  parakeet-tdt-0.6b-v2
    - parakeet-tdt-0.6b-v2-generic-cpu:1                      device=CPU  ep=CPUExecutionProvider
  phi-3-mini-128k
    - Phi-3-mini-128k-instruct-openvino-gpu:2                 device=GPU  ep=OpenVINOExecutionProvider
    - Phi-3-mini-128k-instruct-generic-gpu:2                  device=GPU  ep=WebGPUExecutionProvider
    - Phi-3-mini-128k-instruct-generic-cpu:3                  device=CPU  ep=CPUExecutionProvider
  phi-3-mini-4k
    - Phi-3-mini-4k-instruct-openvino-npu:2                   device=NPU  ep=OpenVINOExecutionProvider
    - Phi-3-mini-4k-instruct-openvino-gpu:2                   device=GPU  ep=OpenVINOExecutionProvider
    - Phi-3-mini-4k-instruct-generic-gpu:2                    device=GPU  ep=WebGPUExecutionProvider
    - Phi-3-mini-4k-instruct-generic-cpu:3                    device=CPU  ep=CPUExecutionProvider
  phi-3.5-mini
    - Phi-3.5-mini-instruct-openvino-gpu:2                    device=GPU  ep=OpenVINOExecutionProvider
    - Phi-3.5-mini-instruct-generic-gpu:2                     device=GPU  ep=WebGPUExecutionProvider
    - Phi-3.5-mini-instruct-generic-cpu:2                     device=CPU  ep=CPUExecutionProvider
  phi-4
    - phi-4-openvino-gpu:2                                    device=GPU  ep=OpenVINOExecutionProvider
    - Phi-4-generic-gpu:2                                     device=GPU  ep=WebGPUExecutionProvider
    - Phi-4-generic-cpu:2                                     device=CPU  ep=CPUExecutionProvider
  phi-4-mini
    - phi-4-mini-instruct-openvino-npu:4                      device=NPU  ep=OpenVINOExecutionProvider
    - phi-4-mini-instruct-openvino-gpu:2                      device=GPU  ep=OpenVINOExecutionProvider
    - Phi-4-mini-instruct-generic-gpu:5                       device=GPU  ep=WebGPUExecutionProvider
    - Phi-4-mini-instruct-generic-cpu:5                       device=CPU  ep=CPUExecutionProvider
  phi-4-mini-reasoning
    - Phi-4-mini-reasoning-openvino-npu:4                     device=NPU  ep=OpenVINOExecutionProvider
    - Phi-4-mini-reasoning-openvino-gpu:2                     device=GPU  ep=OpenVINOExecutionProvider
    - Phi-4-mini-reasoning-generic-gpu:3                      device=GPU  ep=WebGPUExecutionProvider
    - Phi-4-mini-reasoning-generic-cpu:3                      device=CPU  ep=CPUExecutionProvider
  phi-4-reasoning
    - Phi-4-reasoning-generic-gpu:1                           device=GPU  ep=WebGPUExecutionProvider
    - Phi-4-reasoning-generic-cpu:1                           device=CPU  ep=CPUExecutionProvider
  qwen2.5-0.5b
    - qwen2.5-0.5b-instruct-openvino-npu:5                    device=NPU  ep=OpenVINOExecutionProvider
    - qwen2.5-0.5b-instruct-openvino-gpu:2                    device=GPU  ep=OpenVINOExecutionProvider
    - qwen2.5-0.5b-instruct-generic-gpu:4                     device=GPU  ep=WebGPUExecutionProvider
    - qwen2.5-0.5b-instruct-generic-cpu:4                     device=CPU  ep=CPUExecutionProvider
  qwen2.5-1.5b
    - qwen2.5-1.5b-instruct-openvino-npu:5                    device=NPU  ep=OpenVINOExecutionProvider
    - qwen2.5-1.5b-instruct-openvino-gpu:2                    device=GPU  ep=OpenVINOExecutionProvider
    - qwen2.5-1.5b-instruct-generic-gpu:4                     device=GPU  ep=WebGPUExecutionProvider
    - qwen2.5-1.5b-instruct-generic-cpu:4                     device=CPU  ep=CPUExecutionProvider
  qwen2.5-14b
    - qwen2.5-14b-instruct-openvino-gpu:2                     device=GPU  ep=OpenVINOExecutionProvider
    - qwen2.5-14b-instruct-generic-gpu:4                      device=GPU  ep=WebGPUExecutionProvider
    - qwen2.5-14b-instruct-generic-cpu:4                      device=CPU  ep=CPUExecutionProvider
  qwen2.5-7b
    - qwen2.5-7b-instruct-openvino-npu:4                      device=NPU  ep=OpenVINOExecutionProvider
    - qwen2.5-7b-instruct-openvino-gpu:2                      device=GPU  ep=OpenVINOExecutionProvider
    - qwen2.5-7b-instruct-generic-gpu:4                       device=GPU  ep=WebGPUExecutionProvider
    - qwen2.5-7b-instruct-generic-cpu:4                       device=CPU  ep=CPUExecutionProvider
  qwen2.5-coder-0.5b
    - qwen2.5-coder-0.5b-instruct-openvino-npu:5              device=NPU  ep=OpenVINOExecutionProvider
    - qwen2.5-coder-0.5b-instruct-openvino-gpu:2              device=GPU  ep=OpenVINOExecutionProvider
    - qwen2.5-coder-0.5b-instruct-generic-gpu:4               device=GPU  ep=WebGPUExecutionProvider
    - qwen2.5-coder-0.5b-instruct-generic-cpu:4               device=CPU  ep=CPUExecutionProvider
  qwen2.5-coder-1.5b
    - qwen2.5-coder-1.5b-instruct-openvino-npu:5              device=NPU  ep=OpenVINOExecutionProvider
    - qwen2.5-coder-1.5b-instruct-openvino-gpu:2              device=GPU  ep=OpenVINOExecutionProvider
    - qwen2.5-coder-1.5b-instruct-generic-gpu:4               device=GPU  ep=WebGPUExecutionProvider
    - qwen2.5-coder-1.5b-instruct-generic-cpu:4               device=CPU  ep=CPUExecutionProvider
  qwen2.5-coder-14b
    - qwen2.5-coder-14b-instruct-openvino-gpu:2               device=GPU  ep=OpenVINOExecutionProvider
    - qwen2.5-coder-14b-instruct-generic-gpu:4                device=GPU  ep=WebGPUExecutionProvider
    - qwen2.5-coder-14b-instruct-generic-cpu:4                device=CPU  ep=CPUExecutionProvider
  qwen2.5-coder-7b
    - qwen2.5-coder-7b-instruct-openvino-npu:4                device=NPU  ep=OpenVINOExecutionProvider
    - qwen2.5-coder-7b-instruct-openvino-gpu:2                device=GPU  ep=OpenVINOExecutionProvider
    - qwen2.5-coder-7b-instruct-generic-gpu:4                 device=GPU  ep=WebGPUExecutionProvider
    - qwen2.5-coder-7b-instruct-generic-cpu:4                 device=CPU  ep=CPUExecutionProvider
  qwen3-0.6b
    - qwen3-0.6b-generic-gpu:2                                device=GPU  ep=WebGPUExecutionProvider
    - qwen3-0.6b-generic-cpu:4                                device=CPU  ep=CPUExecutionProvider
  qwen3-1.7b
    - qwen3-1.7b-generic-gpu:2                                device=GPU  ep=WebGPUExecutionProvider
    - qwen3-1.7b-generic-cpu:2                                device=CPU  ep=CPUExecutionProvider
  qwen3-14b
    - qwen3-14b-generic-gpu:2                                 device=GPU  ep=WebGPUExecutionProvider
    - qwen3-14b-generic-cpu:2                                 device=CPU  ep=CPUExecutionProvider
  qwen3-4b
    - qwen3-4b-generic-gpu:2                                  device=GPU  ep=WebGPUExecutionProvider
    - qwen3-4b-generic-cpu:3                                  device=CPU  ep=CPUExecutionProvider
  qwen3-8b
    - qwen3-8b-generic-gpu:2                                  device=GPU  ep=WebGPUExecutionProvider
    - qwen3-8b-generic-cpu:2                                  device=CPU  ep=CPUExecutionProvider
  qwen3-embedding-0.6b
    - qwen3-embedding-0.6b-generic-gpu:1                      device=GPU  ep=WebGPUExecutionProvider
    - qwen3-embedding-0.6b-generic-cpu:1                      device=CPU  ep=CPUExecutionProvider
  qwen3-embedding-8b
    - qwen3-embedding-8b-generic-gpu:1                        device=GPU  ep=WebGPUExecutionProvider
    - qwen3-embedding-8b-generic-cpu:1                        device=CPU  ep=CPUExecutionProvider
  qwen3-vl-2b-instruct
    - qwen3-vl-2b-instruct-generic-cpu:2                      device=CPU  ep=CPUExecutionProvider
  qwen3-vl-4b-instruct
    - qwen3-vl-4b-instruct-generic-cpu:3                      device=CPU  ep=CPUExecutionProvider
  qwen3-vl-8b-instruct
    - qwen3-vl-8b-instruct-generic-cpu:2                      device=CPU  ep=CPUExecutionProvider
  qwen3.5-0.8b
    - qwen3.5-0.8b-generic-gpu:4                              device=GPU  ep=WebGPUExecutionProvider
    - qwen3.5-0.8b-generic-cpu:3                              device=CPU  ep=CPUExecutionProvider
  qwen3.5-2b
    - qwen3.5-2b-generic-gpu:4                                device=GPU  ep=WebGPUExecutionProvider
    - qwen3.5-2b-generic-cpu:3                                device=CPU  ep=CPUExecutionProvider
  qwen3.5-2b-text
    - qwen3.5-2b-text-generic-gpu:3                           device=GPU  ep=WebGPUExecutionProvider
    - qwen3.5-2b-text-generic-cpu:1                           device=CPU  ep=CPUExecutionProvider
  qwen3.5-4b
    - qwen3.5-4b-generic-gpu:4                                device=GPU  ep=WebGPUExecutionProvider
    - qwen3.5-4b-generic-cpu:3                                device=CPU  ep=CPUExecutionProvider
  qwen3.5-9b
    - qwen3.5-9b-generic-gpu:4                                device=GPU  ep=WebGPUExecutionProvider
    - qwen3.5-9b-generic-cpu:3                                device=CPU  ep=CPUExecutionProvider
  smollm3-3b
    - smollm3-3b-generic-gpu:1                                device=GPU  ep=WebGPUExecutionProvider
    - smollm3-3b-generic-cpu:1                                device=CPU  ep=CPUExecutionProvider
  whisper-base
    - openai-whisper-base-generic-cpu:3                       device=CPU  ep=CPUExecutionProvider
  whisper-large-v3-turbo
    - openai-whisper-large-v3-turbo-generic-cpu:4             device=CPU  ep=CPUExecutionProvider
  whisper-medium
    - openai-whisper-medium-generic-cpu:4                     device=CPU  ep=CPUExecutionProvider
  whisper-small
    - openai-whisper-small-generic-cpu:4                      device=CPU  ep=CPUExecutionProvider
  whisper-tiny
    - openai-whisper-tiny-generic-cpu:4                       device=CPU  ep=CPUExecutionProvider

== Streaming speech model ==
  nemotron-speech-streaming-en-0.6b: resolved (1 variants)
~~~


### Spike 1: loopback capture (cpal 0.15.3, this machine)
- cpal input stream on the default output device works as WASAPI loopback: "Speakers (Realtek(R) Audio)", 2 ch, 48 kHz, f32.
- Test: 6 s capture while playing Windows `Ring05.wav` -> 198,240 mono frames (~4.1 s, not 6 s), peak 14306/32767.
  Frames only arrive while something is playing (silence produces no packets), so the capture loop must be timestamp-based, not frame-count-based, to keep Me/Them transcript timing right.
- Output `spikes/loopback-test.wav` is 16 kHz mono 16-bit via a naive box-filter resampler. **Not yet listened to** - needs a human check; a YouTube clip test is still open. Replace with rubato if it sounds aliased.
- cpal worked, so the `wasapi` crate fallback was not needed.

### Spike 2: dual live sessions (SDK 2.1.0, nemotron-speech-streaming-en-0.6b, CPU, this machine)
- **Two concurrent sessions on one loaded model work.** Two `AudioSession`s fed in real time from two synthetic (Windows SAPI) 16 kHz speech WAVs ("Me"/"Them") both produced complete, accurate text, interleaved, finishing in 10.1 s for ~9 s of audio each. Model download ~600 MB; load 1.0-1.3 s once cached. Mixed-stream fallback not needed.
- **The 1.x `LiveAudioTranscriptionSession` is deprecated in 2.x.** Use `AudioSession::new(&model)` + `session.create_input_queue()` + `process_streaming_request(Request::from_items([audio]).with_input_queue(q))`; read output items from the `ItemStream`.
- Undocumented native requirements (found by error messages):
  1. The request's first item must be `Item::audio_data(.., format "pcm", 16000, 1)` (an empty request fails: "First item must be AUDIO").
  2. Format string must be exactly `pcm` (not `pcm16`).
  3. Items pushed to the queue must be `Item::bytes(raw PCM16 LE)`, not Audio items.
- **Output is token-level deltas, not utterances.** Every item is an `Item::SpeechSegment` with `kind = None` (never `Partial`/`Final`), text is a sub-word piece (leading space marks a new word). No `SpeechResult` was emitted. "Only write final results" therefore needs our own utterance segmentation (sentence punctuation / pause timing / `utterance_start` / segment timestamps) - to design in Phase 1.
- Not yet tested: real mic + real loopback simultaneously, CPU cost of two sessions, NPU (no NPU variant of this model exists).
