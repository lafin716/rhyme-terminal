# Mobile terminal keyboard fix

Cause: App.vue always constructed xterm with disableStdin=true and registered no onData handler. The composer worked but terminal keyboard input never reached MobileClient.

Reproduction: Chromium touch-sized viewport, authenticate/attach mocked transport, click terminal and dispatch a,b,Enter. Before fix zero input messages; component regressions failed on disabled input and missing listener.

Fix: track disableStdin from authenticated attachment availability, register onData and forward keys without awaiting previous acknowledgments, suppress stale/disconnected terminal input. Preserve composer path.

Verification: 170 frontend tests passed; production frontend build and cargo build --bin winmux using target-mobile passed. Chromium click and touch-tap routes each produced exactly a,b,CR; composer still sent one command. Physical phone keyboard not exercised.

Binary: target-mobile/debug/winmux.exe. Restart desktop app with this build and pair again to receive updated embedded assets. Existing user daemon was not killed or restarted.
