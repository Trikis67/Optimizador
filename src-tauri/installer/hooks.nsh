; Personalización del instalador con la marca Trikis.
; Tauri incluye este archivo antes de definir las páginas, así que aquí podemos cambiar
; colores y textos de Modern UI (MUI2).

; Colores de la marca en las páginas de bienvenida/final y en el encabezado
!define MUI_BGCOLOR "1A1916"
!define MUI_TEXTCOLOR "EFE8D6"
!define MUI_HEADERIMAGE_RIGHT

; Bienvenida
!define MUI_WELCOMEPAGE_TITLE "Bienvenido al Optimizador de Trikis"
!define MUI_WELCOMEPAGE_TEXT "Este asistente instalará el Optimizador $\r$\n(versión ${VERSION}) en tu PC.$\r$\n$\r$\n\
Monitor de rendimiento, limpieza de RAM y disco, ajustes para juegos, drivers y salud del sistema en un solo lugar.$\r$\n$\r$\n\
Todo lo que la app cambia se puede deshacer y queda anotado en el Historial.$\r$\n$\r$\n\
Haz clic en Siguiente para continuar."

; Final
!define MUI_FINISHPAGE_TITLE "¡Listo! El Optimizador está instalado"
!define MUI_FINISHPAGE_TEXT "La app se abre siempre como administrador para poder aplicar los ajustes del sistema, \
así que Windows te pedirá permiso cada vez que la abras.$\r$\n$\r$\nCreado por Gabriel · trikis.nl"
!define MUI_FINISHPAGE_LINK "Visitar trikis.nl"
!define MUI_FINISHPAGE_LINK_LOCATION "https://trikis.nl/"
!define MUI_FINISHPAGE_LINK_COLOR "D19A3C"

; ---------------------------------------------------------------------------------------
; Páginas internas oscuras. Recorre los controles de la página y les aplica los colores de la
; marca. Las casillas y marcos pierden el tema visual de Windows para que el texto claro se vea;
; los botones normales (Examinar, Ver detalles) se dejan como están.
; ---------------------------------------------------------------------------------------
!macro TRIKIS_DARK_PAGE UN
Function ${UN}TrikisDarkPage
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  FindWindow $R0 "#32770" "" $HWNDPARENT
  SetCtlColors $R0 EFE8D6 1A1916
  StrCpy $R1 0
  trikis_loop:
    FindWindow $R1 "" "" $R0 $R1
    StrCmp $R1 0 trikis_done
    System::Call 'user32::GetClassNameW(p R1, w .R2, i 64)'
    StrCmp $R2 "Button" 0 trikis_paint
    System::Call 'user32::GetWindowLongW(p R1, i -16) i .R3'
    IntOp $R3 $R3 & 0xF
    ; 2/3 casilla, 4/9 opción, 7 marco: se pintan. El resto son botones: se dejan.
    StrCmp $R3 2 trikis_untheme
    StrCmp $R3 3 trikis_untheme
    StrCmp $R3 4 trikis_untheme
    StrCmp $R3 9 trikis_untheme
    StrCmp $R3 7 trikis_untheme
    Goto trikis_loop
  trikis_untheme:
    System::Call 'uxtheme::SetWindowTheme(p R1, w " ", w " ")'
  trikis_paint:
    StrCmp $R2 "Edit" 0 +3
      SetCtlColors $R1 EFE8D6 2D2A24
      Goto trikis_loop
    SetCtlColors $R1 EFE8D6 1A1916
    Goto trikis_loop
  trikis_done:
  ; Barra de progreso ámbar (los colores van en formato BGR)
  GetDlgItem $R1 $R0 1004
  StrCmp $R1 0 +4
    System::Call 'uxtheme::SetWindowTheme(p R1, w " ", w " ")'
    SendMessage $R1 0x0409 0 0x003C9AD1 ; PBM_SETBARCOLOR
    SendMessage $R1 0x2001 0 0x00242A2D ; PBM_SETBKCOLOR
  ; Lista de detalles de la instalación
  GetDlgItem $R1 $R0 1016
  StrCmp $R1 0 +4
    SendMessage $R1 0x1001 0 0x00242A2D ; LVM_SETBKCOLOR
    SendMessage $R1 0x1026 0 0x00242A2D ; LVM_SETTEXTBKCOLOR
    SendMessage $R1 0x1024 0 0x00D6E8EF ; LVM_SETTEXTCOLOR
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
FunctionEnd
!macroend
!insertmacro TRIKIS_DARK_PAGE ""
!insertmacro TRIKIS_DARK_PAGE "un."

!macro NSIS_HOOK_POSTINSTALL
  ; Nada extra por ahora: los accesos directos y el registro los crea Tauri.
!macroend
