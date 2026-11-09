# Fixtures: `dashscope`

QwenCloud's DashScope video synthesis: `POST /api/v1/services/aigc/video-generation/video-synthesis` (and `/api/v1/services/aigc/image2video/video-synthesis` for Wan 2.2 and 2.1 first and last frames), and `GET /api/v1/tasks/{task_id}`.

## Sources

- https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task, `…/wan-image-to-video-first-frame/create-task`, `…/wan27-text-to-video/create-task` and `…/query-result`, `…/wan27-image-to-video/create-task`, `…/wan30-video/create-task` and `…/query-result`, the image-to-video first and last frame guide, and https://docs.qwencloud.com/api-reference/more/manage-asynchronous-tasks, read 2026-11-09.
- No fixture is recorded: video synthesis is paid, and no QwenCloud key was used.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). `polls/` holds the provider's answers to a task's status call.

## Files

| file | what it covers | provenance |
|---|---|---|
| `errors/invalid-api-key-401.json` | an error, with its status in `.meta.json` | doc: DashScope error response (`InvalidApiKey`) |
| `errors/rate-quota-429.json` | an error, with its status in `.meta.json` | doc: asynchronous tasks (`Throttling.RateQuota`) |
| `polls/failed.json` | a failed task | doc: Wan 2.7 query result, `FAILED` example |
| `polls/running.json` | a running task | doc: Wan 2.7 query result, `RUNNING` example |
| `polls/succeeded.json` | a finished task with its video and usage | doc: Wan 2.7 query result, `SUCCEEDED` example |
| `polls/unknown.json` | an expired task | doc: Wan 2.7 query result, `UNKNOWN` example |
| `requests/wan22-first-last-frame.json` | Wan 2.2 first and last frames | doc: first and last frame guide |
| `requests/wan26-first-frame.json` | Wan 2.6 Flash image to video with sound | doc: first-frame create task |
| `requests/wan26-text-to-video.json` | Wan 2.6 text to video with a size and audio | doc: text-to-video create task |
| `requests/wan27-first-last-frame.json` | Wan 2.7 frames in `media` | doc: Wan 2.7 image-to-video create task |
| `requests/wan27-text-to-video.json` | Wan 2.7 text to video with a tier and ratio | doc: Wan 2.7 text-to-video create task |
| `requests/wan30-reference-images.json` | Wan 3.0 reference images | doc: Wan 3.0 create task |
| `responses/started.json` | the reply that starts a task | doc: `AsyncTaskSubmitResponse` |
