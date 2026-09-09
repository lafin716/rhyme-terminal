# Task 2 mobile browser report

## 구현 파일

- `mobile.html`
- `vite.mobile.config.ts`
- `src/mobile/App.vue`
- `src/mobile/client.ts`
- `src/mobile/client.test.ts`
- `src/mobile/encoding.ts`
- `src/mobile/invitation.ts`
- `src/mobile/main.ts`
- `src/mobile/style.css`
- `src/mobile/types.ts`
- `package.json`
- `.gitignore`

`pnpm-lock.yaml`은 새 의존성이 없어 변경하지 않았다.

## 구현 결과

- URL fragment의 초대 토큰을 읽은 즉시 주소에서 제거하며, 명시적인 새 초대는 탭에 남아 있는 이전 기기 토큰보다 우선한다.
- 기기 토큰은 `sessionStorage`에만 저장한다. 승인 대기 확인 번호, 인증 오류, 연결 및 재연결 상태를 한국어로 표시한다.
- 인증 후 세션 목록, attach/detach 전환, snapshot 초기화, live PTY 출력, 입력 요청을 지원한다. 세션 추가/삭제/이름 변경 이벤트는 100ms debounce 후 목록을 다시 조회한다.
- 재연결은 0.5/1/2/4/8초로 최대 5회만 시도한다. 전송 결과가 불확실한 입력은 큐에 저장하거나 재전송하지 않는다.
- socket generation과 snapshot sequence로 이전 연결의 메시지, 응답, 비동기 terminal 초기화를 차단한다. snapshot 적용 중 live 출력은 순서대로 최대 1MiB만 버퍼링하며 초과 시 연결을 닫아 재동기화한다.
- 입력은 UTF-8 16KiB 이하로 제한해 서버의 24KiB JSON 메시지 제한 안에 유지한다. 한글 IME 조합 중 Enter 중복 전송을 막고, composer Enter/전송은 CR을 붙여 명령을 실행한다. Enter, Tab, Esc, Ctrl+C 보조 키도 제공한다.
- xterm은 PC가 반환한 cols/rows를 그대로 사용하며 모바일 resize 요청을 보내지 않는다. 작은 화면에서는 터미널 영역 내부를 스크롤한다.
- 모바일 번들은 Tauri import 없이 별도 entry/type으로 구성하며 `dist-mobile/mobile.html`과 상대 `assets/*`를 만든다. `dev`와 `build`가 모바일 자산을 먼저 준비한다.

## 검증

- `pnpm.cmd test`: 성공. 26 test files, 166 tests passed. 모바일 단위 테스트는 fragment 제거, 새 초대 우선, 승인 대기, 토큰 저장, 목록 조회, attach/detach 전환, snapshot 전달, UTF-8 입력 제한, transport loss 입력 비재전송, stale socket 무시를 검증한다.
- `pnpm.cmd exec vue-tsc --noEmit`: 성공.
- `pnpm.cmd build`: 성공. 모바일 19 modules와 데스크톱 1279 modules를 빌드했다. 데스크톱 번들의 기존 대형 chunk 경고만 있었다.
- `pnpm.cmd build:mobile`: 최종 소스에서 성공. `dist-mobile/mobile.html`, `assets/mobile-DxNXKPOh.css`, `assets/mobile-CcaK9ubt.js` 생성.
- controller Chromium mock smoke: timer receiver 수정 후 성공. 터미널 출력, `echo test`의 CR 1회 전송, 390px viewport의 문서 가로 overflow 없음과 screenshot을 확인했다. 마지막 snapshot generation guard 번들에 대한 재실행은 controller가 수행한다.
- `git diff --check -- .gitignore package.json mobile.html vite.mobile.config.ts src/mobile`: 오류 없음. Windows CRLF 전환 안내만 출력됐다.

## 제한 및 후속 확인

- 실제 휴대전화와 Tailscale 네트워크를 통한 QR 접속은 사용 가능한 기기가 필요해 이 작업에서 확인하지 않았다.
- controller가 완성된 backend와 최종 번들로 positive WebSocket/PTY 통합 테스트 및 최신 Chromium smoke를 수행한다.
- 서버가 제공하는 terminal cell 치수와 모바일 브라우저의 글꼴 렌더링 차이 때문에 실제 기기에서 긴 줄의 시각적 정렬은 한 번 확인해야 한다.
