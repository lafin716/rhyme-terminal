# Rhyme Flow 사용 및 검증

기존 앱 중앙의 **Rhyme Flow** 버튼으로 엽니다. Terminal 전환은 터미널 세션이나
Flow 초안을 제거하지 않습니다. 숨겨진 Flow 화면은 목록을 polling하지 않습니다.

## 시작

1. 기존 Workspace 설정에서 프로젝트 기본 폴더를 지정합니다.
2. Flow 디자이너의 기본 PowerShell → output 예제를 검증하고 **새 버전 확정**을 누릅니다.
3. 프로젝트 채팅에서 업무를 저장합니다. 추가 입력 JSON이 있으면 실행 설정에 넣습니다.
   `goal`과 `project`는 엔진이 요구사항과 프로젝트에서 채웁니다.
4. 명령과 권한을 확인하고 **확정 버전 실행**을 누릅니다. 실행 기록에서 승인, 취소,
   단계별 입력·출력·오류, 감사 이벤트와 결과물을 확인합니다.

## 워커와 Flow

워커 탭에서 지침, 입력/출력 계약, 권한과 timeout을 JSON으로 편집할 수 있습니다.
저장은 새 불변 버전을 만듭니다. 삭제는 카탈로그에서 보관 처리하며 이전 Run에서
참조한 버전은 유지합니다. agent 노드의 `definition_ref`에 워커 id와 revision을 지정합니다.

캔버스는 노드 드래그, 추가/삭제, 조건 포트 연결과 노드 JSON 편집을 지원합니다.
전체 JSON으로 inputs/outputs, 정책, 반복 본문도 편집할 수 있습니다. JSON 가져오기는
초안을 바꾸고, 새 버전 확정은 별도 단계입니다. 다른 저장소의 worker 참조를 가져오려면
해당 worker 버전도 먼저 등록해야 합니다. 저장된 최신 버전과 다른 기준으로 저장하면
충돌 오류가 나므로 최신 정의를 불러와 변경을 적용해야 합니다.

자연어 변경은 실제 Codex CLI를 호출합니다. 반환된 JSON을 서버에서 검증하고 현재
정의와 나란히 표시합니다. 사용자가 초안에 적용하고 버전을 확정하기 전에는 실행하지
않습니다. Codex CLI나 인증이 없으면 설정 필요/실행 오류가 표시됩니다.

프로젝트 채팅의 **AI 의도 분리 · 기존 업무 검색**은 기존 Task/Flow를 재사용할 계획을
제안합니다. 계획을 검토한 후 메시지를 저장합니다. 추천 Flow가 있으면 선택됩니다.
AI 없이 저장하면 줄 단위 분리, 동일 문장 중복 확인, 명시적으로 선택한 Task 연결을
사용합니다. 이는 의미 기반 AI 분류로 표시되지 않습니다. 아이디어는 메시지로 유지하며
Task를 만들지 않습니다. 최근 메시지는 채팅 화면에서 다시 볼 수 있습니다.

추가 요구사항은 새 RequirementRevision과 pending Instruction으로 저장합니다.
진행 중 Run의 요구사항은 바뀌지 않으며 **다음 실행**에 반영됩니다.

## DSL v1

Rust `flow/model.rs`가 표준 직렬화·검증 계약입니다. Vue의 `flow-types.ts`가 같은
필드를 사용하며 `flow-contract.fixture.json`을 양쪽 테스트가 읽습니다.

- 입력 바인딩: `{ "source": "$inputs" 또는 선행 node id, "path": "/JSON/pointer" }`.
- 명령 입력: 바인딩 객체를 JSON stdin으로 전달합니다. `config.stdin_binding`으로
  바인딩 이름을 지정하면 해당 값만 전달합니다. 문자열은 그대로, 나머지는 JSON입니다.
  바인딩을 셸 명령문에 보간하지 않습니다. 명령은 정적 `program`과 문자열 `args` 배열입니다.
- condition: `config.binding`으로 입력 이름을 선택하고 `equals`와 비교합니다.
  출력 `{ "value": boolean }`에 따라 `true`/`false` 포트를 활성화합니다.
- 반복: `max_attempts` 1–10, 직렬 `body`, `until: {node,path,equals}`.
  중첩 반복과 반복 내부 승인·Git 액션은 금지합니다.
- 계약: `type`, `properties`, `required`, `items`, `enum`, boolean `additionalProperties`,
  `description`, `title`을 지원합니다. 다른 키는 거절합니다. Flow outputs 계약은
  node id를 키로 하는 최종 결과 객체에 적용합니다.
- 병렬도 1–8, node timeout 1–3600초, Run timeout 1–86400초입니다.

## 개발 패키지

**개발 패키지**를 누르면 기존 공통 엔진으로 실행하는 개발자·검토자와 Flow 초안을
설치합니다. Worktree → 개발/검증/리뷰 반복 → 승인 → 커밋 → Draft PR 순서입니다.

실행 전에 verify의 기본 `pnpm.cmd build`를 프로젝트에 맞게 바꾸고,
commit의 `files`에 검토할 파일 경로를 명시하세요. 기본 빈 파일 목록은 커밋되지 않습니다.
의존성 설치가 필요하면 검토한 명령 노드를 추가하세요. 부모 작업 디렉터리의 미커밋 파일은
Worktree로 복사하지 않으며 원본을 수정하지 않습니다.

Codex 로그인, Git 작성자 설정, origin 및 GitHub CLI 인증은 기존 설정을 사용합니다.
이 기능 구현 과정에서 현재 개발 저장소를 push하거나 실제 PR을 만들지는 않았습니다.

게시 액션은 `external_publish` 실행 권한 선택과 실제 대상·커밋에 대한 단계 승인을
모두 요구합니다. 실패/종료 후 **원격 PR 결과 대조**로 기존 브랜치의 원격 결과를 확인합니다.
이미 PR이 있으면 또 만드는 대신 해당 결과를 검토해야 합니다. 자동 병합·배포는 없습니다.

`command`는 사용자 권한으로 실행되는 신뢰된 로컬 프로그램입니다. 파일·네트워크에
접근할 수 있으므로 JSON으로 가져온 명령은 권한 부여 전에 검토해야 합니다.

## 수명주기와 데이터

데이터는 Tauri local-data 경로의 `flow/flow.sqlite3`에 저장됩니다. 화면을 전환하거나
창을 트레이로 숨겨도 앱 프로세스가 살아 있으면 실행은 계속됩니다. **앱 프로세스 종료
후에도 실행된다고 보장하지 않습니다.** 프로세스 종료 시 하위 프로세스를 종료하고,
다음 시작 때 미완료 Run을 interrupted로 대조합니다. 재시도는 새 Run입니다.

Flow DB 손상/잠금은 Flow 오류로 표시되며 기존 터미널 시작을 막지 않습니다.
DB를 삭제해 복구하지 않습니다. 다른 앱 인스턴스가 Flow를 소유한 경우 그 인스턴스를
확인해야 합니다. 설정·인증 파일과 기존 localStorage는 그대로 유지합니다.

## 검증 명령

```powershell
pnpm.cmd test
pnpm.cmd build
cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=2
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

자동 테스트는 순환/참조/반복 검사, 불변 버전, 낙관적 충돌 검사, 메시지 중복과 요구사항
이력, 재시작 대조, 조건 분기, 병렬 실행, 전체 반복 재실행, 명령 stdin, 비정상 종료·취소·
timeout·출력 상한, 공백/한글 경로, Windows 하위 프로세스 종료, Git Worktree 격리와
Codex JSON 이벤트 실패를 검증합니다. Codex 인증 응답과 GitHub 실제 Draft PR 생성은
외부 서비스 설정 없이는 실검증되지 않습니다.
