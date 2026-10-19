!macro NSIS_HOOK_POSTUNINSTALL
  MessageBox MB_YESNO "Delete all Talent Scout resumes and scores stored by this app?" /SD IDNO IDNO skip_delete
  RMDir /r "$APPDATA\com.talentscout.desktop"
  RMDir /r "$LOCALAPPDATA\com.talentscout.desktop"
  skip_delete:
!macroend
