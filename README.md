# my_streamio

Experimental multi-swarm torrent streaming project.

## CI 실행과 결과 확인

GitLab `main`에 푸시하면 보호된 `main`과 태그를 GitHub 공개 미러에 동기화합니다. 워크플로는 `main` 푸시에서 시작합니다. GitHub-hosted Runner가 중앙 `ci-hub` 워크플로에서 README의 공백 오류와 충돌 표시를 검사합니다. [GitHub Actions 실행 로그](https://github.com/giinii06/my_streamio-ci-mirror/actions/workflows/gitlab-bridge.yml)

- 이전 실행을 다시 요청하려면 GitHub Actions에서 **Re-run jobs**를 선택합니다.
- GitLab 커밋 상세에서 같은 SHA의 `github-actions/ci-hub` 상태를 확인합니다. GitHub에서 완료하면 호스팅된 컨트롤러가 결과를 GitLab에 반영합니다.
- Rust 코드를 `main`에 병합할 때 `Cargo.lock`을 커밋하고 [호출 워크플로](.github/workflows/gitlab-bridge.yml)의 `stack`을 `rust`로 지정합니다. 그러면 러너가 Rust 워크스페이스의 빌드와 테스트를 실행합니다.

### ChatGPT 웹에서 CI 요청

GitLab 플러그인에서 이 프로젝트의 브랜치 또는 태그를 지정해 파이프라인을 생성합니다. GitHub 컨트롤러가 해당 SHA를 검사한 뒤 필요한 이력을 공개 미러에 전달하고 CI를 예약합니다. GitLab 파이프라인에서 `github-actions/ci-hub-request`의 상태와 GitHub 실행 링크를 확인합니다. [프로젝트 공통 CI 안내](https://github.com/giinii06/ci-hub#chatgpt-웹에서-ci-요청)

### CI 병렬 실행

my_streamio의 CI는 다른 프로젝트의 CI와 동시에 실행할 수 있습니다. 한 프로젝트에서 여러 푸시가 각각 실행을 시작하면 그 실행들도 겹칠 수 있습니다. 러너가 모두 사용 중이면 실행은 대기열에 들어갑니다. [워크플로 동시 실행](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency)

현재 실행마다 작업 하나를 사용합니다. 독립 검사를 병렬로 처리하려면 중앙 `ci-hub` 워크플로에서 별도 `jobs`로 나눕니다. 같은 검사를 여러 버전이나 운영체제에서 실행하려면 `strategy.matrix`를 사용합니다. [작업 의존성](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-jobs) · [매트릭스 실행](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/run-job-variations)
