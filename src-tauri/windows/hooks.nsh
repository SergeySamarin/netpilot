; EN: NSIS hooks of the NetPilot installer (see "installerHooks" in tauri.conf.json).
; RU: Хуки установщика NetPilot на NSIS (см. «installerHooks» в tauri.conf.json).

!macro NSIS_HOOK_PREUNINSTALL
  ; EN: An update also runs the old uninstaller, with /UPDATE — then nothing is cleaned up.
  ;     On a real removal NetPilot removes its autostart task and asks what to do with the zapret service.
  ; RU: Обновление тоже запускает старый деинсталлятор, с /UPDATE — тогда ничего не убираем.
  ;     При настоящем удалении NetPilot убирает свою задачу автозапуска и спрашивает, что делать со службой zapret.
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/UPDATE" $R1
  ${If} ${Errors}
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall'
  ${EndIf}
!macroend
