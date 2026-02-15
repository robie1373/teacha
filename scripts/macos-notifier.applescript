on run argv
    if (count of argv) >= 2 then
        set theTitle to item 1 of argv
        set theBody to item 2 of argv
    else
        set theTitle to "teacha"
        set theBody to "Reminder"
    end if

    display notification theBody with title theTitle
end run
