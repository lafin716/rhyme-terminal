# Agent Loop — Runtime 기반 터미널 관리

Loop는 사용자가 직접 실행한 Coding Agent를 관리한다. 별도 채팅 UI나 실행 프롬프트 창이 아니다.

## 1. 기존 구현 분석

기존 LoopCreateDialog는 신규 프롬프트 또는 기존 대화를 선택하고 useLoopRouting.create에 시작 방식을 전달했다. loop_routing/runtime.rs는 Agent를 PTY 루트로 만들고 전환마다 새 PTY를 생성했다. Usage 초과 시 switch_pending과 tool hook 경계를 기다렸으며 도구·하위 Agent·출력 유휴와 실행 상태가 섞여 있었다.

일반 터미널 경로는 PaneTabs/useSessions → Tauri → daemon IPC → spawn_session → PTY다. Terminal.vue → write_session → WriteSession 입력과 PTY 출력 이벤트는 그대로 사용한다. 일반 Agent 생성과 resume는 실행 인수만 다르다.

좌측 패널에는 두 신호가 있었다.

- 프로세스 존재/Provider: ipc/server.rs의 process_snapshot → SessionManager::refresh_agents → pty/agent.rs의 shell 자손 탐색 → SessionAgentChanged → useSessions → SideBar.
- turn 작업 상태: AgentTaskTracker가 PTY 입력·출력/Claude 제목을 관찰하여 SessionAgentStatusChanged를 보낸다.

첫 번째 경로를 Runtime Monitor로 추출했다. 합성 Loop 행의 활동 표시는 실제 PTY의 기존 tracker를 참조한다. Agent process와 turn 완료를 구분한다.

## 2. 신규/기존 선택 제거

LoopCreateDialog.vue에서 신규/기존 선택, 대화 검색, 프롬프트 입력을 제거했다. lib/loop-routing.ts의 LoopStart/대화 선택 타입과 composable의 conversations 요청도 제거했다. Adapter의 대화 검색·검증은 handoff에 재사용한다.

생성 창은 이름, 참여 계정, 임계값, 전략, 조회 간격·interrupt 시간·자동 재개만 설정한다. 그룹별 정책을 저장하고 현재 계정 레지스트리의 인증/활성화 정보를 결합한다. 환경변수 값은 Loop DB에 직렬화하지 않는다.

## 3. Terminal lifecycle

Create → shell PTY 하나 생성 → RESUMING → 활성 목록 첫 프로필의 Provider 명령 자동 전달 → PREPARING → fresh Usage guard → Agent child 실행이다. 최초 실행은 전체 candidates 순서대로 검증하며 사용할 수 없는 프로필은 다음 순번으로 넘어간다. 이후 전환은 선택 전략을 따른다. 종료 후 IDLE에서는 사용자가 codex/codex resume/claude/claude --resume를 직접 실행할 수 있다. 전환 뒤에도 동일 PTY·화면·working directory를 유지한다. git status/npm test/dir 같은 일반 명령은 Loop 시작 신호가 아니다.

한 turn 완료나 process exit를 Task 완료로 처리하지 않는다. 자연 종료는 IDLE이고 Loop/shell은 살아 있다. 별도 Task Mode가 없어 COMPLETED는 도입하지 않았다.

## 4. 공통 Runtime Monitor

pty/runtime_monitor.rs는 IDLE/STARTING/RUNNING/INTERRUPTING/EXITED/ERROR, Provider, PID, 시작 시각을 다룬다. Source of truth는 **OS 프로세스 snapshot과 PTY shell 자손 관계**다. executable과 Node/command wrapper 위치를 조합하며 Prompt 문자열로 running을 판정하지 않는다.

pty/agent.rs::ProcessTree를 snapshot마다 한 번 만들고 sidebar 감지와 Loop 종료 대상 탐색이 공유한다. 사라진 세션의 캐시를 정리하고 PID 재사용은 시작 시각으로 구분한다. Loop UI는 같은 snapshot을 그룹 조회로 전달받는다.

## 5. 실행 전 Usage Guard

bridge.rs가 Loop shell에 codex/claude 함수를 설치한다. 함수는 argv를 JSON으로 전달하는 배포 바이너리의 helper 모드를 호출한다.

helper 요청 → PREPARING → refresh_account_usage로 캐시 우회 → enabled/executable/fresh/threshold 미만 검증 → 후보 선택 → **활성화 직전 새 조회** → 승인 → child spawn 순이다. 조회 실패/오래된 표본을 0%로 처리하지 않는다. Controller heartbeat가 사라지면 실행을 거부한다.

절대 경로 실행처럼 함수를 우회하면 process tree 감지 직후 interrupt하고 검증 경로로 돌아온다. 재구성 가능한 executable/Node argv와 실제 cwd를 보존한다. 임의 복합 shell 표현식은 추측하여 재실행하지 않는다.

## 6. Safe Boundary 제거와 상태 전환

runtime의 tool boundary 대기 상태/타이머와 Windows/Unix hook의 tool 경계 보류를 제거했다. SessionStart 신원 확인 gate만 유지한다. 잘못된 native resume가 빈 세션으로 진행되는 것을 막는 startup 검증이며 tool 완료 대기가 아니다.

RUNNING → SWITCHING_PROFILE → 입력 큐 → Ctrl+C → Agent 및 포착한 자손 종료 확인 → HANDOFF → RESUMING → PREPARING → Usage 재확인 → 승인 → PID 확인 → RUNNING.

기본 interrupt 5초 후 terminate, 추가 3초 후 kill을 요청한다. 종료를 확인하지 못하면 다음 실행을 차단한다. Windows에서는 terminate가 OS 프로세스 종료 요청으로 동작할 수 있다. PTY shell을 교체하지 않는다.

## 7. Profile Scheduler

enabled/executable/fresh Usage/단기·주간 threshold를 우선 검증한다. SMART는 낮은 사용량 → 높은 priority → 오래 사용하지 않은 계정 순이다. LEAST_USAGE/PRIORITY/ROUND_ROBIN도 지원한다. 선택 후 87%에서 91%로 바뀌면 건너뛴다.

구조화된 rate-limit 이벤트에는 cooldown을 적용한다. Context limit은 같은 Profile의 새 Session continuation이며 Usage/rate limit과 별도 이벤트다. 실행 중 계정의 조회를 우선하고 결과를 해당 계정의 Loop에 즉시 반영한다.

## 8. Native Resume / Logical Handoff

신규와 resume는 동일 dispatch/runtime lifecycle을 통과한다. UUID가 명시된 resume는 adapter가 참여 Profile에서 확인하고 정확한 대화 기록만 가져온다. 인증 폴더 전체를 복사하지 않는다. 첫 startup ID 검증 후 CLI 내부의 다른 conversation 선택을 허용한다.

현재 Session ID의 기록만 사용하여 이전의 다른 작업을 이어받지 않는다. 같은 Provider의 native resume를 우선하고 불가능하거나 startup 검증에 실패하면 한 번 logical handoff로 전환한다.

Logical context는 최초 요청, 최근 지시, 기록된 진행 내용, 도구 실행/결과·미확인 작업, branch/worktree, git status, staged/unstaged diff를 포함한다. transcript/git 출력 크기와 git 실행 시간을 제한한다. 전체 대화·숨겨진 추론·인증 객체를 무제한 복사하지 않고 완료 여부를 추측하지 않는다. Continuation은 현재 파일부터 확인하고 처음부터 다시 시작하지 말도록 지시한다. 작업이 있었는데 context를 확보하지 못하면 빈 작업을 자동 시작하지 않는다.

## 9. 입력 보존 / 중복 실행 방지

자동 continuation은 startup 신원 gate를 거치는 CLI 초기 프롬프트로 전달한다. 큰 context는 실행별 handoff.md에 저장해 Windows 명령행 길이 제한을 피한다. CLI의 로그인·프로젝트 신뢰 화면에서는 직접 입력할 수 있다. 전환 입력은 UUID byte queue로 저장한다. Startup 확인 후 continuation → queued input 순서로 보낸다. 전송 직전 delivering checkpoint, 성공 뒤 제거한다. 비정상 종료로 결과가 불확실하면 입력을 보관하고 PAUSED로 복구한다. 사용자가 표시된 입력의 전달 여부를 확인하기 전에는 자동 재전송하지 않는다.

Manager lock 안에서 전환/승인을 직렬화한다. interrupt 전 포착한 PID+시작 시각과 Agent/helper 존재를 확인하고 새 snapshot으로 종료가 확인되어야 다음 Agent를 승인한다.

## 10. Ctrl+C / Pause / Resume / Stop

| 조작 | 의미 |
|---|---|
| Terminal Ctrl+C | CLI로 전달. 실제 Agent 종료 시 IDLE. turn만 취소하고 살아 있으면 계속 관리 |
| Pause | Agent interrupt 후 PAUSED. 자동 선택/재개 없음. shell 사용 가능 |
| Resume | Usage refresh 및 Profile 재선택. 이전 Agent가 남아 있으면 먼저 종료 확인 |
| Stop | Agent interrupt, 예약 취소, STOPPED 영속화. 종료 확인 지연에도 자동 재개 없음 |

PAUSED/STOPPED에서 사용자가 실행하는 CLI는 자동 관리하지 않는다. UI는 backend command만 호출하고 선택/kill/handoff를 직접 수행하지 않는다.

## 11. 모두 소진된 경우

실패 대신 WAITING_FOR_USAGE_RESET이다. 한 Profile의 소진 window들은 모두 reset되어야 하므로 가장 늦은 reset을 쓰고, Profile들 중 가장 이른 시간을 선택한다. waitingProfileId/resumeAt과 countdown을 저장/표시한다. 시간이 불명확하면 설정 간격 후 재확인한다.

시간 도달 후 실제 Usage를 새로 조회한다. 여전히 불가능하면 다른 Profile/새 시간을 선택한다. shell 입력 중이거나 다른 명령이 실행 중이면 자동 명령 삽입을 미룬다. Pause/Stop 뒤 예약은 실행되지 않는다.

## 12. Persistence / 복원

routing.sqlite3에 그룹/정책, terminal ID, runtime snapshot, active Profile, Usage, Provider/Session ID, handoff, 대기 시간, 입력 큐, 최근 200개 System Event를 저장한다. 동일 상태 반복 DB 쓰기와 일반 키 입력당 DB 쓰기를 제거했다.

UI 재연결은 살아 있는 daemon Runtime을 사용한다. daemon 재시작은 이전 PID/PTY를 RUNNING으로 복구하지 않는다. IDLE/PAUSED/WAITING을 유지하고 불확실한 실행은 IDLE로 복원하여 runtime을 관찰한다. STOPPED는 자동 shell/Agent 실행 대상이 아니다. 새 정책/Runtime metadata가 없는 이전 버전의 WAITING 기록은 PAUSED로 이관해 잘못된 Provider의 빈 작업이 자동 실행되지 않게 한다. 기존 Session 식별자는 보존하며 명시적 Resume에서 재검증한다.

## 13. UI / 주요 파일

기본 접힌 status bar에 상태·활성 계정·Usage·Pause/Stop·Latest Event 한 줄, 나머지 화면에는 Terminal.vue를 표시한다. 상세 패널에는 Profile별 progress bar/threshold marker/remaining/reset/상태, session/runtime, timeline과 고급 수동 전환이 있다. 상태 dropdown과 기본 '다음 계정'은 제거했다. 시스템 로그를 Agent 출력에 주입하지 않는다.

- src-tauri/src/pty/{agent,runtime_monitor,manager}.rs: 공통 runtime.
- src-tauri/src/loop_routing/{runtime,model,bridge,process_controller,adapter,mod}.rs: 상태·guard·선택·프로세스·handoff·저장.
- src-tauri/src/usage.rs: fresh Usage.
- src-tauri/scripts/loop-shell.ps1, src-tauri/scripts/loop-routing-hook.ps1, src-tauri/src/unix_cli.rs: startup/session hook.
- src-tauri/src/{main,bin/winmuxd}.rs: helper 진입.
- src/components/{LoopCreateDialog,LoopGroupView,LoopRoutingSettings,SideBar,PaneTabs}.vue, src/composables/useLoopRouting.ts, src/lib/loop-routing.ts: 생성·표시·명령·탐색기 연결.

## 14. 검증

runtime_tests.rs는 자동 실행 예약과 최초 순서/activation 재검증, 일반 shell, codex/resume gate, inclusive threshold, 즉시 interrupt, escalation, 종료 전 중복 실행 금지, Ctrl+C/Pause/Stop, reset 재검증, 입력 한 번 전달, activation 직전 threshold 변경, 재시작/입력 보존을 다룬다. monitor/PID 재사용과 adapter transcript/비밀정보 제외/startup hook 검사도 포함한다.

브라우저 mock QA는 실제 빌드에서 세션 선택 제거, 생성 자동 실행 안내, 접힌 기본 상태, 활성 계정, 그래프/countdown 및 1440px/390px overflow를 검사한다. 별도 Rust ConPTY 검사는 GUI subsystem으로 바꾼 테스트 바이너리에서 실제 입력·출력·shell 복귀를 검사한다. Windows helper QA는 실제 바이너리에서 승인 전 미실행/취소/resume argv·특수문자 보존을 검사한다. 테스트 child를 사용하며 사용자 daemon/인증을 변경하지 않는다.

검증 명령:

~~~powershell
pnpm.cmd exec vitest run --maxWorkers=2
pnpm.cmd build
cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1
cargo build --manifest-path src-tauri/Cargo.toml --bins
node .scratch/agent-loop-runtime-qa/run.mjs
node .scratch/agent-loop-runtime-qa/bridge.mjs
~~~

## 15. 기술적 제약

2026-09-10 검증 결과: 프런트엔드 42개 파일/266개 테스트, Rust 전체 150개 테스트, 이후 추가 회귀를 포함한 최종 Loop 40개 테스트가 통과했다. 설치된 Codex hook 설정 검사와 실제 ConPTY/GUI-subsystem helper 검사도 포함한다. pnpm build, cargo build --bins, 변경 프런트엔드 ESLint, git diff --check, 1440px/390px 브라우저 QA 및 helper 승인/취소/빈 인수 검사가 통과했다. 부하가 있는 병렬 실행에서는 기존 Flow 테스트의 15초 제한에 걸려 최종 전체 검증은 test-threads=1로 수행했다.

빌드 중 사용자가 실행한 winmuxd 프로세스는 종료하지 않았다. 잠긴 실행 이미지는 target/debug/winmuxd-running-b92a9819c83c4e43ade7adc5541fd3c5.exe로 보관하고 정식 이름의 새 바이너리를 빌드했다. 실행 중인 프로세스는 그대로 유지되며 다음 시작부터 새 파일을 사용한다.

- Usage는 Provider 표본이다. polling 사이 소비를 미리 알 수 없어 서버가 threshold에 도달하는 물리적 순간까지 보장하지 않는다. **확인된 threshold는 즉시 중단**한다.
- shell 함수 우회는 process snapshot(약 1초) 이후 감지한다. 임의 절대 경로/별도 shell/복합 wrapper의 실행 전 완전한 OS interception은 아니다.
- native resume는 CLI hook/대화 형식과 계정 ownership에 의존한다. UUID 없는 picker는 선택된 Profile에서 동작한다.
- Context/rate/usage 구분은 Provider가 구조화된 오류 코드를 보낼 때 가능하다. 임의 터미널 문자열을 quota 오류로 추측하지 않는다.
- snapshot 사이에 생성되어 즉시 분리된 외부/원격 작업까지 소유권을 보장하지 않는다. 알려진 로컬 자손은 포착해 종료 확인한다.
- Claude setup-token의 statusline 표본 대기 및 Claude Usage 조회 실패는 실행 후 확인으로 전환한다. 실행 전 refresh 시도는 유지하되 알려진 미초기화 임계값 초과 또는 실제 오류 cooldown이 없으면 실행을 허용한다. 실행 중에도 조회 실패만으로 중단하지 않고 폴링을 계속한다. Codex의 기존 조회 방어는 유지한다.
- PTY에는 Provider 공통 durable ACK가 없다. 비정상 종료 경계의 exactly-once는 보관/사용자 확인으로 해결한다.
- 실제 유료 계정 간 quota→native resume→reset 재개와 Unix native UI는 별도 실환경 검증이 필요하다. 테스트 helper/PTY와 mock 브라우저가 이를 대신하지 않는다.
- 기존 daemon을 자동 교체하지 않는다. 새 runtime capability가 없는 daemon에는 안내를 표시한다.

설정의 전체 프로필 영역은 Provider 구분 없는 활성 / 대기 카드 목록으로 구성한다. Claude Code / Codex는 카드 태그로 표시한다. 행 앞 핸들로 전체 실행 순서를 바꾸거나 영역 사이로 이동한다. 카드의 위아래 위치와 숫자는 Provider에 관계없는 전역 순번이며 방향키와 활성/대기 버튼도 지원한다. 대기 프로필은 생성 창에 표시하지 않는다. 새 daemon의 autoStart capability를 확인하여 오래된 daemon에서 조용히 일반 shell만 생성하는 동작을 방지한다.

2026-09-10 활성 프로필 설정 / 자동 시작 변경 검증:
- `pnpm build`, 대상 ESLint, `cargo build --bin winmuxd` 통과.
- Loop Rust 회귀 테스트 40개 통과, 설치 CLI / GUI ConPTY 전용 테스트 2개는 이번 실행에서 제외.
- 브라우저 `.scratch/agent-loop-runtime-qa/profiles.mjs`: Provider 두 열, 영역 간 드래그, 전체 순서 저장, 활성 프로필만 생성 창에 표시, 1440px / 390px 레이아웃 및 오류 없음 확인.
- 사용자 daemon은 중단하지 않았다. 새 자동 시작 capability를 적용하려면 기존 작업 종료 후 daemon을 새 바이너리로 재시작해야 한다. 실제 계정 인증 및 과금 Usage를 사용하는 실행은 이번 검증에 포함하지 않았다.

루프별 실시간 설정:
- 상세 패널에서 참여 프로필의 단기/주간 임계값과 우선순위를 수정하며, 변경 시 해당 Loop 정책만 저장한다.
- 선택 전략, 사용량 조회 간격, reset 후 자동 재개 역시 Loop별로 수정한다. 전역 기본 설정과 다른 Loop에는 전파하지 않는다.
- `update_policy`는 허용 필드만 받으며, Engine lock 안에서 현재 `activeProfile`의 임계값 변경을 거부한다. UI도 두 임계값을 잠금 처리한다. 계정 전환과 경합해 요청이 거부되면 화면 입력은 저장된 값으로 복원한다.
- WAITING 중 수정은 재검증 일정을 앞당기되 실제 Usage Guard를 통과해야 재개된다. PAUSED/STOPPED 상태는 유지한다.
- 정책과 이벤트를 기존 SQLite에 저장하고 `livePolicy` capability가 없는 daemon에는 재시작 안내를 표시한다.
- `.scratch/agent-loop-runtime-qa/live-policy.mjs`로 임계값 수정/그래프 갱신, 활성 잠금, 전역 기본값 격리, 좁은 화면을 검증했다.

프로필 카드 드래그 수정:
- 기존 HTML DragEvent 직접 호출 검사는 통과했지만, CDP 마우스 누름/이동/놓기에서는 dragstart가 발생하지 않고 재정렬에 실패하는 현상을 재현했다.
- LoopRoutingSettings는 Pointer capture와 좌표 기반 삽입 위치 판정으로 변경했다. 위/아래 삽입선, 영역 간 이동, 스크롤 가장자리 자동 이동, Esc/blur/cancel/unmount 정리를 포함한다.
- `.scratch/agent-loop-runtime-qa/pointer-drag.mjs`는 DragEvent를 직접 호출하지 않고 실제 브라우저 마우스 입력을 보내 Codex 1번 이동, 활성/대기 왕복, 저장 순서를 검사한다. 수정 전 실패, 수정 후 통과했다. 빌드와 대상 ESLint도 통과했다.

Claude 실행 후 Usage 확인 경로:
- usagePending 표시로 조회 대기를 오류와 구분한다. 표본을 확보하면 정상 임계값 검사로 복귀한다.
- Claude 전용 StopFailure hook을 추가했다. API error 문자열 및 한도 오류 진단만 정규화하며, 대화/도구 출력의 문구는 검사하지 않는다. 실제 quota/rate limit은 기존 interrupt → 종료 확인 → handoff → 다음 프로필 경로로 전환한다. 인증 실패도 cooldown 후 다음 프로필을 확인한다.
- 공식 형식: https://code.claude.com/docs/en/hooks#stopfailure

Claude Usage fallback 검증:
- Loop 회귀 테스트 48개 통과(설치 CLI / GUI ConPTY 전용 2개 제외), 추가 실행 승인 테스트 1개 통과.
- 91% 계정을 건너뛰고 사용량 샘플이 없는 setup-token 계정에 launch response를 내리는 경로를 임시 UUID 프로필과 가짜 토큰으로 확인했다. 테스트 프로필은 생성 시 기존 디렉터리가 없음을 확인하고 종료 시 정리한다. 실제 Claude 작업/API 호출은 하지 않는다.
- 실제 PowerShell hook에서 StopFailure 정규화와 원본 진단문 미저장을 확인했다. 샘플 대기 중 RUNNING 유지, 실제 threshold/usage limit 전환, cooldown 유지도 검사했다.
- pnpm build, 대상 ESLint, cargo build --bin winmuxd, usage-pending.mjs 브라우저 검사 통과. 실행 중 사용자 daemon은 유지했다.
