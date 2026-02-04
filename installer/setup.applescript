-- Little Helper Setup
-- Double-click this app to install everything

on run
	-- Show welcome dialog
	set dialogResult to display dialog "Welcome to Little Helper!

This will install:
• Little Helper app
• Ollama AI engine
• AI model (~2GB download)

Your Mac password will be needed once.
Installation takes about 5 minutes." buttons {"Cancel", "Install"} default button "Install" with title "Little Helper Setup" with icon note

	if button returned of dialogResult is "Install" then
		-- Run the installer
		set scriptPath to (POSIX path of (path to me)) & "Contents/Resources/install.sh"

		-- Open Terminal and run install script
		tell application "Terminal"
			activate
			do script "bash '" & scriptPath & "'"
		end tell

		display dialog "Installation started in Terminal!

Please follow the prompts there.
When complete, Little Helper will open automatically." buttons {"OK"} default button "OK" with title "Little Helper Setup" with icon note
	end if
end run
