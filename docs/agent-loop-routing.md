# Agent Loop — Runtime 기반 터미널 관리

Loop는 사용자가 직접 실행한 Coding Agent를 관리한다. 별도 채팅 UI나 실행 프롬프트 창이 아니다.

## 1. 무엇을 관리하는가

일반 터미널 경로는 PaneTabs/useSessions → Tauri → daemon IPC → spawn_session → PTY다. Terminal.vue → write_session → WriteSession 입력과 PTY 출력 이벤트는 그대로 쓴다. 일반 Agent 생성과 resume는 실행 인수만 다르다.

좌측 패널에는 두 신호가 있다.

- 프로세스 존재/Provider: ipc/server.rs의 process_snapshot → SessionManager::refresh_agents → pty/agent.rs의 shell 자손 탐색 → SessionAgentChanged → useSessions → SideBar.
- turn 작업 상태: AgentTaskTracker가 PTY 입력·출력/Claude 제목을 관찰하여 SessionAgentStatusChanged를 보낸다.

첫 번째 경로를 Runtime Monitor로 뽑아냈다. 합성 Loop 행의 활동 표시는 실제 PTY의 기존 tracker를 참조한다. Agent process 존재와 turn 완료는 서로 다른 사실로 다룬다.

## 2. 생성은 아무것도 묻지 않는다

Loop를 만들 때 고를 것은 없다. 참여 계정과 순서, 5시간 / 주간 한도 제한, 프로필별 개별 한도, 전환 순서, 사용량 조회 간격은 전부 설정 → 에이전트 루프에 있다. `PaneTabs`의 '새 에이전트 루프'는 저장된 활성 프로필 목록을 참여 계정으로 삼아 즉시 Loop를 만들고, 첫 프로필로 바로 시작한다. 활성 프로필이 하나도 없으면 설정으로 안내한다.

환경변수 값은 Loop DB에 직렬화하지 않는다. Adapter의 대화 검색·검증은 handoff에 재사용한다.

## 3. Terminal lifecycle

Create → shell PTY 하나 생성 → RESUMING → 활성 목록 첫 프로필의 Provider 명령 자동 전달 → PREPARING → 프로필 선택 → Agent child 실행이다. 최초 실행은 설정 순서를 따르며 한도에 걸린 프로필은 다음 순번으로 넘어간다. 이후 전환은 설정의 전환 순서를 따른다. IDLE에서는 사용자가 codex/codex resume/claude/claude --resume를 직접 실행할 수 있다. 전환 뒤에도 동일 PTY·화면·working directory를 유지한다. git status/npm test/dir 같은 일반 명령은 Loop 시작 신호가 아니다.

한 turn이 끝난 것은 작업 완료가 아니다. Agent 프로세스가 스스로 끝난 것이 작업 완료이며, 그때 Loop는 COMPLETED가 되어 관리를 멈춘다(11절).

## 4. 공통 Runtime Monitor

pty/runtime_monitor.rs는 IDLE/STARTING/RUNNING/INTERRUPTING/EXITED/ERROR, Provider, PID, 시작 시각을 다룬다. Source of truth는 **OS 프로세스 snapshot과 PTY shell 자손 관계**다. executable과 Node/command wrapper 위치를 조합하며 Prompt 문자열로 running을 판정하지 않는다.

pty/agent.rs::ProcessTree를 snapshot마다 한 번 만들고 sidebar 감지와 Loop 종료 대상 탐색이 공유한다. 사라진 세션의 캐시를 정리하고 PID 재사용은 시작 시각으로 구분한다. Loop UI는 같은 snapshot을 그룹 조회로 전달받는다.

## 5. 사용량은 프롬프트가 도는 동안에만 조회한다

Provider의 quota는 Agent가 일하는 동안에만 움직인다. 그래서 `usage_targets`가 Provider에 요청을 보내는 경우는 두 가지뿐이다.

1. 어떤 Loop의 활성 계정이 **프롬프트를 실행 중**일 때(`pending_work`). Agent가 떠 있기만 하고 아직 프롬프트를 받지 않았다면 `pending_work`가 false이므로 조회하지 않는다 — 첫 프로필을 띄운 직후가 정확히 이 상태다.
2. 사용자가 Loop 터미널의 감시 바에서 **지금 확인**을 눌렀을 때.

대기 중인 계정은 quota를 쓰지 않으므로 숫자가 변할 수 없다. 이들을 주기적으로 조회하는 것이 Provider의 usage endpoint를 429로 만들었고, 429는 성긴 표본보다 나쁘다 — 숫자가 아예 없어진다.

실행 전 Usage guard는 없다. 계정을 막는 근거는 **증거**뿐이다.

- `blocked_until`: Provider가 보낸 한도 이벤트, 또는 우리가 받은 표본에서 소진된 창의 reset 시각.
- 마지막 표본이 한도 이상이고 그 창이 아직 reset되지 않음.

조회한 적이 없는 계정은 실행 가능하다. 이것이 첫 Agent가 조회 없이 시작할 수 있는 이유이며, 저장된 reset 시각이 지나면 새 조회 없이도 쿨타임이 끝나는 이유다.

`bridge.rs`가 Loop shell에 설치하는 codex/claude 함수는 그대로다. 함수는 argv를 JSON으로 전달하고 Loop가 승인한 managed launch만 실행한다. 감시를 껐거나 Loop가 일시정지·종료·완료 상태이면 사용자의 명령을 unmanaged로 되돌려주어 셸이 멈추지 않게 한다. Controller heartbeat가 사라지면 실행을 거부한다.

## 6. 상태 전환

RUNNING → (한도 도달) → SWITCHING_PROFILE → 입력 큐 → Ctrl+C → Agent 및 포착한 자손 종료 확인 → HANDOFF → RESUMING → PREPARING → 다음 프로필 선택 → RUNNING.

한도에 도달하면 **즉시** 전환한다. turn 경계를 기다리는 switch_pending 상태는 없앴다. 조회 자체가 프롬프트 실행 중에만 일어나므로 판정 시점이 곧 작업 중이고, 한도를 넘긴 계정으로 계속 진행해 봐야 Provider가 거절한다. 진행 중인 작업은 handoff context로 다음 프로필에 넘어간다.

기본 interrupt 5초 후 terminate, 추가 3초 후 kill을 요청한다. 종료를 확인하지 못하면 다음 실행을 차단한다. PTY shell은 교체하지 않는다.

## 7. Profile Scheduler

`eligible`은 enabled / 실행 파일 존재 / `blocked_until` 경과 / 마지막 표본이 한도 미만(만료되지 않은 창 기준)만 본다. 표본이 없는 것은 실격 사유가 아니고, 조회 실패도 아니다 — 일시적인 조회 실패 한 번이 실행 중인 Loop를 끊고 전환한 다음 프로필까지 같은 이유로 끊는 것이 이전 설계의 실패였다.

임계값 판정에 쓰는 창은 **계정마다** 고른다(`Candidate.thresholdBasis`, 기본 `short`). 기준 창만 설정한 임계값으로 막고, 나머지 창은 100% 소진에서만 막는다.

전환 순서는 설정의 전환 순서를 따른다. ROUND_ROBIN은 현재 계정 다음부터 순환하고, SMART/LEAST_USAGE는 사용량이 적은 계정부터 고른다. 조회한 적이 없는 계정의 사용량은 0%로 본다 — 대기 계정에는 표본이 없는 것이 정상이므로, 이들을 뒤로 밀면 숫자를 가진 한 계정에 Loop가 묶인다.

구조화된 rate-limit / usage-limit 이벤트는 종전대로 해당 프로필을 차단하고 전환을 건다. Context limit은 같은 Profile의 새 Session continuation이다.
## 8. Native Resume / Logical Handoff

신규와 resume는 동일 dispatch/runtime lifecycle을 통과한다. UUID가 명시된 resume는 adapter가 참여 Profile에서 확인하고 정확한 대화 기록만 가져온다. 인증 폴더 전체를 복사하지 않는다. 첫 startup ID 검증 후 CLI 내부의 다른 conversation 선택을 허용한다.

현재 Session ID의 기록만 사용하여 이전의 다른 작업을 이어받지 않는다. 같은 Provider의 native resume를 우선하고 불가능하거나 startup 검증에 실패하면 한 번 logical handoff로 전환한다.

Logical context는 최초 요청, 최근 지시, 기록된 진행 내용, 도구 실행/결과·미확인 작업, branch/worktree, git status, staged/unstaged diff를 포함한다. transcript/git 출력 크기와 git 실행 시간을 제한한다. 전체 대화·숨겨진 추론·인증 객체를 무제한 복사하지 않고 완료 여부를 추측하지 않는다. Continuation은 현재 파일부터 확인하고 처음부터 다시 시작하지 말도록 지시한다. 작업이 있었는데 context를 확보하지 못하면 빈 작업을 자동 시작하지 않는다.

## 9. 입력 보존 / 중복 실행 방지

자동 continuation은 startup 신원 gate를 거치는 CLI 초기 프롬프트로 전달한다. 큰 context는 실행별 handoff.md에 저장해 Windows 명령행 길이 제한을 피한다. CLI의 로그인·프로젝트 신뢰 화면에서는 직접 입력할 수 있다. 전환 입력은 UUID byte queue로 저장한다. Startup 확인 후 continuation → queued input 순서로 보낸다. 전송 직전 delivering checkpoint, 성공 뒤 제거한다. 비정상 종료로 결과가 불확실하면 입력을 보관하고 PAUSED로 복구한다. 사용자가 표시된 입력의 전달 여부를 확인하기 전에는 자동 재전송하지 않는다.

Manager lock 안에서 전환/승인을 직렬화한다. interrupt 전 포착한 PID+시작 시각과 Agent/helper 존재를 확인하고 새 snapshot으로 종료가 확인되어야 다음 Agent를 승인한다.

## 10. Ctrl+C / Pause / Resume / Stop / 작업 완료

| 조작 | 의미 |
|---|---|
| Terminal Ctrl+C | CLI로 전달. 실제 Agent 종료 시 IDLE. turn만 취소하고 살아 있으면 계속 관리 |
| Pause | Agent interrupt 후 PAUSED. 자동 선택/재개 없음. shell 사용 가능 |
| Resume | Profile 재선택 후 재개. 이전 Agent가 남아 있으면 먼저 종료 확인. COMPLETED에서도 누를 수 있다 |
| 작업 완료 | COMPLETED로 표시하고 관리를 끝낸다. 터미널은 살아 있다 |
| Stop | Agent interrupt, 예약 취소, STOPPED 영속화. 종료 확인 지연에도 자동 재개 없음 |

PAUSED/STOPPED/COMPLETED에서 사용자가 실행하는 CLI는 자동 관리하지 않는다. UI는 backend command만 호출하고 선택/kill/handoff를 직접 수행하지 않는다.

## 11. 작업 완료 — Loop 종료

Agent가 **스스로 종료**하면 이 Loop가 맡은 작업이 끝난 것이다. Loop는 COMPLETED가 되어 더 이상 프로필을 고르지도, 사용량을 조회하지도 않는다. 터미널은 그대로 두어 결과를 읽을 수 있게 한다.

끝난 것과 죽은 것은 bridge helper가 남기는 `exit-{dispatchId}.json`의 종료 코드로 구분한다. 0(또는 기록이 없음)은 COMPLETED, 그 외는 IDLE이며 이유를 표시한다. Loop가 건 interrupt나 사용자의 Ctrl+C로 인한 종료는 여기 해당하지 않는다 — 각각 SWITCHING_PROFILE과 IDLE 경로를 이미 지나간다.

## 12. 모두 소진된 경우 — 쿨타임

실패 대신 WAITING_FOR_USAGE_RESET이다. 한 Profile의 소진 window들은 모두 reset되어야 하므로 그 중 **가장 늦은** reset을 쓰고, Profile들 중 **가장 이른** 시간을 선택한다(`cooldown_until`). waitingProfileId/resumeAt과 countdown을 저장/표시한다. 시간이 불명확하면 설정 간격 후 재확인한다.

쿨타임 자체에는 조회가 필요 없다. 계정은 자신이 보고한 reset 시각까지 막혀 있고, 그 시각이 지났다는 사실이 곧 다시 쓸 수 있다는 증거다. 시간이 되어도 아직 풀린 프로필이 없으면 새로 계산한 reset으로 대기를 다시 건다 — 이전 설계는 여기서 매 tick마다 셸에 provider 명령을 다시 쳐 넣었다. shell 입력 중이거나 다른 명령이 실행 중이면 자동 명령 삽입을 미룬다. Pause/Stop 뒤 예약은 실행되지 않는다.

## 13. Persistence / 복원

routing.sqlite3에 그룹/정책, terminal ID, runtime snapshot, active Profile, **계정별 quota(표본·reset·`blocked_until`)**, Provider/Session ID, handoff, 대기 시간, 입력 큐, 최근 200개 System Event를 저장한다. quota를 저장하는 이유는 쿨타임이 reset 시각으로만 계산되기 때문이다 — 재시작이 그 시각을 잃으면 Loop는 아직 풀리지 않은 한도에 Agent를 다시 밀어 넣는다. 동일 상태 반복 DB 쓰기와 일반 키 입력당 DB 쓰기는 하지 않는다.

UI 재연결은 살아 있는 daemon Runtime을 사용한다. daemon 재시작은 이전 PID/PTY를 RUNNING으로 복구하지 않는다. IDLE/PAUSED/WAITING/COMPLETED를 유지하고 불확실한 실행은 이어갈 Session이 있으면 PAUSED로, 없으면 IDLE로 복원한다. STOPPED/COMPLETED는 자동 shell/Agent 실행 대상이 아니다. 기존 Session 식별자는 보존하며 명시적 Resume에서 재검증한다.

## 14. UI / 주요 파일

생성은 아무것도 묻지 않는다. 계정·한도·전환 순서·조회 간격은 모두 설정 → 에이전트 루프에 있고, `PaneTabs`의 '새 에이전트 루프'는 저장된 활성 프로필 목록으로 즉시 Loop를 만든다.

Loop 터미널 위에는 두 줄이 있다.

- **status bar**: 상태·활성 계정·Usage·일시정지/작업 완료/종료·Latest Event.
- **감시 바**: 감시 on/off 토글, Agent 프로세스 존재 여부(PID), 프롬프트 실행 여부, 사용량 조회 상태와 '지금 확인'. 세 신호를 따로 보여주는 이유는 상태 한 단어로는 구분되지 않기 때문이다 — Agent가 떠 있는가, 프롬프트가 도는가, Loop가 개입해도 되는가. 감시를 끄면 관찰과 표시는 계속하되 중단·전환·조회를 하지 않고, 셸 gate에 걸린 명령은 unmanaged로 돌려준다.

상세 패널에는 Profile별 progress bar/threshold marker/remaining/reset/마지막 확인 시각, session/runtime, timeline과 고급 수동 전환이 있다. 시스템 로그를 Agent 출력에 주입하지 않는다.

- src-tauri/src/pty/{agent,runtime_monitor,manager}.rs: 공통 runtime.
- src-tauri/src/loop_routing/{runtime,model,bridge,process_controller,adapter,mod}.rs: 상태·선택·프로세스·handoff·저장.
- src-tauri/src/usage.rs: Usage 조회와 프로필별 단일 슬롯 캐시.
- src-tauri/scripts/loop-shell.ps1, src-tauri/scripts/loop-routing-hook.ps1, src-tauri/src/unix_cli.rs: startup/session hook.
- src-tauri/src/{main,bin/winmuxd}.rs: helper 진입.
- src/components/{LoopGroupView,LoopRoutingSettings,SideBar,PaneTabs}.vue, src/composables/useLoopRouting.ts, src/lib/loop-routing.ts: 생성·표시·명령·탐색기 연결.

## 15. 검증

runtime_tests.rs는 자동 실행 예약, 설정 순서에 따른 최초 선택과 소진 계정 건너뛰기, 일반 shell, codex/resume gate, inclusive threshold, 한도 도달 즉시 전환, 조회 실패가 실행을 끊지 않음, 프롬프트 실행 중에만 조회, 감시 on/off, 지금 확인, escalation, 종료 전 중복 실행 금지, Ctrl+C/Pause/Stop, 자발적 종료 = 작업 완료, 종료 코드로 오류 구분, 저장된 reset 기반 쿨타임과 재무장, 재시작 시 quota 보존, 입력 한 번 전달을 다룬다. monitor/PID 재사용과 adapter transcript/비밀정보 제외/startup hook 검사도 포함한다.

검증 명령:

~~~powershell
pnpm.cmd exec vitest run --maxWorkers=2
pnpm.cmd build
cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1
cargo build --manifest-path src-tauri/Cargo.toml --bins
~~~

## 16. 설정 화면

전체 프로필 영역은 Provider 구분 없는 활성 / 대기 카드 목록이다. Claude Code / Codex는 카드 태그로 표시한다. 행 앞 핸들로 전체 실행 순서를 바꾸거나 영역 사이로 옮긴다. 카드의 위아래 위치와 숫자는 Provider에 관계없는 전역 순번이며 방향키와 활성/대기 버튼도 지원한다. 드래그는 HTML DragEvent가 아니라 Pointer capture와 좌표 기반 삽입 위치 판정을 쓴다 — CDP 마우스 입력에서 dragstart가 발생하지 않아 재정렬이 실패했다.

전역 한도 제한(5시간 / 주간 %)은 모든 프로필의 기본값이다. 프로필마다 `개별 기준`에서 따로 지정할 수 있고, 비워 두면 전역 값을 쓴다.

계정별 사용량 기준(5시간 / 주간):
- 한 계정을 언제 넘길지는 5시간 창과 주간 창 중 하나로 판정한다. 구독마다 소진 방식이 다르므로 Loop 단위가 아니라 **계정 단위** 설정이며, 각 프로필의 `개별 기준 → 사용량 기준`에서 고른다. 기본값은 5시간이다.
- 기준이 아닌 창은 무시하지 않는다. 100%로 소진되면 Provider가 실행 자체를 거부하므로 그 창의 한도는 100%로 유지한다(`runtime.rs`의 `limit`). 임계값 입력은 값을 지우지 않고 흐리게만 처리해, 기준을 되돌리면 쓰던 숫자가 그대로 돌아온다.
- 프로필 카드의 사용량/임계값은 "지금 이 계정을 가장 먼저 막을 창"을 표시한다(`limit` 대비 비율이 가장 큰 창, `ProfileSnapshot.thresholdKind`). 기준이 주간이어도 5시간 창이 95%면 그 숫자를 보여주고, 카드의 창 배지가 어느 창인지 밝힌다.

루프별 실시간 설정:
- 상세 패널에서 참여 프로필의 사용량 기준, 5시간/주간 한도와 우선순위를 수정하며, 변경 시 해당 Loop 정책만 저장한다.
- 전환 순서, 사용량 조회 간격, reset 후 자동 재개 역시 Loop별로 수정한다. 전역 기본 설정과 다른 Loop에는 전파하지 않는다.
- `update_policy`는 허용 필드만 받으며, Engine lock 안에서 현재 `activeProfile`의 사용량 기준과 한도 변경을 거부한다 — 진행 중인 turn 아래에서 판정 기준이 바뀌기 때문이다. UI도 세 입력을 함께 잠근다. 계정 전환과 경합해 요청이 거부되면 화면 입력은 저장된 값으로 복원한다.
- 정책과 이벤트는 기존 SQLite에 저장하고, `livePolicy` capability가 없는 daemon에는 재시작 안내를 표시한다.

## 17. 기술적 제약

- Usage는 Provider 표본이다. 조회 사이의 소비를 미리 알 수 없어 서버가 한도에 닿는 물리적 순간까지 보장하지 않는다. **확인된 한도는 즉시 중단**한다.
- 표본이 없는 것은 실격 사유가 아니다. 계정을 막는 근거는 Provider가 보고한 한도(`blocked_until`)와 아직 reset되지 않은 창의 표본뿐이다. 조회 실패 한 번으로 실행 중인 Loop를 끊으면 전환한 다음 프로필도 같은 이유로 끊기며, 그것이 이전 설계가 무너진 지점이다.
- Provider는 Usage endpoint 자체에 조회 빈도 제한을 건다. 429는 "우리가 너무 자주 물었다"는 뜻이지 계정 quota 소진이 아니므로 다음 조회 시각만 미루고 프로필 자격은 건드리지 않는다. 어떤 경우에도 `MIN_INTERVAL_SECS`(60초)보다 자주 조회하지 않는다.
- Provider 요청은 `usage.rs`의 프로필별 슬롯 하나로 모인다. 화면 표시용 조회와 Loop scheduler 조회가 서로의 표본을 재사용하고, 429를 받으면 cooldown이 끝날 때까지 수동 새로고침을 포함한 모든 경로가 요청을 멈춘다. 막힌 endpoint를 계속 두드리는 것이 제한을 유지시키는 원인이다.
- `usagePending`은 아직 한 번도 읽지 못한 프로필을 뜻한다. 오류가 아니다 — 프롬프트가 도는 동안에만 조회하므로 대기 계정의 정상 상태다. 한 번이라도 읽은 뒤 발생한 오류는 감추지 않고 카드에 노출한다.
- shell 함수 우회는 process snapshot(약 1초) 이후 감지한다. 임의 절대 경로/별도 shell/복합 wrapper의 실행 전 완전한 OS interception은 아니다.
- native resume는 CLI hook/대화 형식과 계정 ownership에 의존한다. UUID 없는 picker는 선택된 Profile에서 동작한다.
- Context/rate/usage 구분은 Provider가 구조화된 오류 코드를 보낼 때 가능하다. 임의 터미널 문자열을 quota 오류로 추측하지 않는다. Claude 전용 StopFailure hook은 API error 문자열과 한도 오류 진단만 정규화하며 대화/도구 출력의 문구는 검사하지 않는다(https://code.claude.com/docs/en/hooks#stopfailure).
- 작업 완료 판정은 Agent 프로세스의 자발적 종료와 helper가 남긴 종료 코드에 의존한다. CLI가 한도 때문에 코드 0으로 끝나면서 hook 이벤트도 남기지 않으면 전환이 아니라 완료로 보인다. 실제로는 hook이 먼저 기록되고 `read_events`가 같은 tick에서 먼저 처리한다.
- snapshot 사이에 생성되어 즉시 분리된 외부/원격 작업까지 소유권을 보장하지 않는다. 알려진 로컬 자손은 포착해 종료 확인한다.
- PTY에는 Provider 공통 durable ACK가 없다. 비정상 종료 경계의 exactly-once는 보관/사용자 확인으로 해결한다.
- 실제 유료 계정 간 quota → native resume → reset 재개와 Unix native UI는 별도 실환경 검증이 필요하다. 테스트 helper/PTY와 mock 브라우저가 이를 대신하지 않는다.
- 기존 daemon을 자동 교체하지 않는다. 새 runtime capability가 없는 daemon에는 안내를 표시한다.

## 18. 2026-09-16 재설계 검증

- `cargo test --lib -- --test-threads=1` 190개 통과(Loop 74개 포함, 설치 CLI / GUI ConPTY 전용 2개 ignored).
- `pnpm exec vitest run --maxWorkers=2` 43개 파일 / 276개 테스트 통과, `pnpm build`, 변경 파일 ESLint, `vue-tsc --noEmit` 통과.
- `cargo build --bin winmuxd --bin winmuxctl` 통과. 실행 중인 사용자 GUI가 `rhyme-terminal.exe`를 잠그고 있어 전체 `--bins` 빌드는 하지 않았고, 사용자 프로세스는 종료하지 않았다. 새 daemon capability를 쓰려면 기존 작업을 마친 뒤 daemon을 재시작해야 한다.
- 실제 계정 인증과 과금 Usage를 사용하는 실행은 이번 검증에 포함하지 않았다.