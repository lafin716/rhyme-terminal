# 모바일 원격 앱 — 원격 연결 목록 (Scoped feature)

## 목표
Android 앱(`src/native-mobile/`)을 "PC 원격 전용 클라이언트"로 완성한다.
- 메인 화면 = 저장된 원격 PC 연결 목록.
- 자체 터미널 없음: 원격 PC에 연결되지 않은 상태에서는 목록 관리·연결 외 기능이 없다.
- 연결하면 PC가 제공하는 모바일 client(`src/mobile/`)에서 작업하고, 언제든 목록으로 돌아온다.

## 완료 기준 (관찰 가능)
1. 앱 실행 시 원격 연결 목록이 보인다. 비어 있으면 빈 상태 안내 + 초대 링크 추가 폼.
2. 초대 링크를 붙여넣고 연결하면 PC origin이 목록에 저장(또는 기존 항목 갱신)되고 PC 화면으로 이동한다.
3. 목록 항목: 이름·주소·마지막 연결 시각, `연결`, `이름 변경`, `삭제`.
4. 저장 항목으로 재연결하면 초대 없이 PC origin으로 이동 — 같은 앱 세션의 기기 인증(PC origin sessionStorage)이 남아 있으면 바로 붙고, 없으면 PC client가 "새 초대 필요" 오류와 `연결 목록` 복귀 버튼을 보인다.
5. PC client 상단 버튼 문구 `다른 PC 연결` → `연결 목록`(동작 동일). 오류 카드에서도 native일 때 `연결 목록`으로 복귀 가능.

## 유지할 동작 / 보안 경계
- 초대 코드·기기 토큰은 앱 저장소에 **저장하지 않는다**(일회용 초대 2분, 토큰은 PC origin sessionStorage). 저장 항목은 `{origin, name, lastConnectedAt}`만(origin이 식별자).
- 초대 링크 검증 규칙(`gatewayInvitation`) 유지. 저장 origin은 http/https + host[:port]만 허용, 경로/쿼리/계정 금지.
- Tauri capabilities 비어 있음, CSP 변경 없음, Rust 변경 없음.
- localStorage 키 `winmux:mobile-remotes:v1`, envelope `{ version: 1, remotes: [...] }`. 손상/다른 버전 데이터는 무시(빈 목록).

## 소유권 / 구현
| 파일 | 내용 |
|---|---|
| `src/native-mobile/remotes.ts` (신규) | 순수 store: load/save/upsertFromInvitation/rename/remove/touch/target URL, 정렬(최근 연결 순) |
| `src/native-mobile/remotes.test.ts` (신규) | 저장 형식·검증·중복 origin 갱신·손상 데이터·target URL |
| `src/native-mobile/App.vue` | 목록 화면 + 추가 폼 + 이름 변경/삭제 확인 |
| `src/mobile/App.vue`, `App.test.ts` | 복귀 버튼 문구/오류 카드 복귀 |
| `docs/mobile-pairing.md`, `docs/ci-cd.md` | 문서 동기화 |

## 난이도 / 모델
MEDIUM(STANDARD/medium). 실행: 리더(현재 Claude Opus 5.5) 직접 구현, 독립 리뷰는 별도 general-purpose agent(모델 상속). 별도 tier 선택 불필요.

## 검증
- `pnpm test`, `pnpm typecheck`, `pnpm lint`, `pnpm build:android`, `pnpm build:mobile`.
- UI: `native-mobile.html`을 Vite dev 서버로 띄워 360×740, 412×915, 768×1024에서 빈/목록/오류/이름변경 상태 스크린샷 확인.
- 실제 Android 기기/에뮬레이터 설치 검증은 로컬 SDK 유무에 따라 수행, 불가 시 Known Issue로 기록.
