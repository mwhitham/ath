# Upgrade from version 1.0.0

`ath migrate` is the upgrade command. It previews the step, writes a verified
backup, and asks before it changes the file. The step adds `name`,
`description`, and `text_result` on workouts, adds `workout_route_points`, and
sets `athleticstandard_version` to `2.0.0`.

Existing revision rows, links, attachments, and deletion rows are not rewritten.
If the step fails, restore the backup named in the error. The command does not
delete that backup.

A JSON package marked `1.0.0` can still be read. New exports are marked
`2.0.0`. Packages marked `0.x` are still refused.
