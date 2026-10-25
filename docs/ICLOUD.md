# iCloud save-file syncing

The unsigned release has CloudKit disabled: ordinary sideloading does not authorize access to the project's CloudKit container. Manual save-file import/export is available. Apple Account sign-in alone does not supply the required entitlements.

## Developer configuration

1. Register both apps under your Apple development team. Enable iCloud/CloudKit and authorize both for one shared container you control.
2. Set your team/bundle identifiers in Xcode. Copy `CloudKit.entitlements.example` to a local entitlements file and select it with `CODE_SIGN_ENTITLEMENTS`.
3. Set `PEGGLE_CLOUDKIT_ENABLED=YES`, `PEGGLE_CLOUDKIT_CONTAINER` to your shared container, and `PEGGLE_CLOUDKIT_ENVIRONMENT=Development` (or `Production` after deploying the schema). Provisioning must authorize those capabilities/environment.
4. The private database uses record type `PeggleProgress`, fields `progressJSON` (Bytes/Data) and `schemaVersion` (Int64), and record ID `prototype-unlocks-v1`. Development builds create the record. Deploy the schema through CloudKit Console before using Production provisioning.
5. Enable syncing in both apps and select the local in-game users to link. Use the same Apple Account, container and environment on both devices.

Private device testing used Development CloudKit. Publishing this repository does not activate Production access. The default container identifier references Respring's test configuration; another developer's signing team cannot use it just by copying the identifier.

## What transfers

The app extracts supported data into its own JSON document, merges it, and applies fields to the selected local in-game user's save file. Original game directories are never placed in iCloud.

- Classic and Nights Adventure checkpoints, unlocked stage/level pairs and Adventure completion.
- Cleared and aced levels.
- **Each individual completed challenge**, identified separately in Classic and Nights. The original game calls these entries trophies; this is not just an overall total.

Scores, statistics, names, account settings, replays and shots within unfinished levels stay local. A later Adventure checkpoint starts its level from the beginning. Unfinished challenge attempts do not transfer.

There is one shared document per Apple Account, not a separate document per in-game user. Each device links a local user; names need not match. Linking another user combines completion data. Deletions/resets do not propagate: completed entries merge as a union.

## Automatic behavior

Completed local writes trigger uploads; unchanged data is checked periodically while active. Incoming changes during gameplay offer Later or Apply and Restart. Successful launch syncs are silent; immediate issues use alerts. The card shows the last successful sync time. Conflict-aware updates and bounded retries preserve local save files after failures.
