# iCloud save-file syncing

## Public builds: choose a folder in Files

1. Open Manage Save Files and choose **Choose iCloud Drive Folder**.
2. In Files, create or choose a **Peggle** folder under **iCloud Drive**. Select that same folder on every device, in both Peggle and Peggle HD.
3. Select the intended in-game user and enable **Enable iCloud Sync**. Each installation links one local user; their names can differ. Use separate folders for separate sets of completed levels and challenges.

Public builds use Files-granted folder access and a persistent bookmark. They need normal app signing/sideloading, but do not need CloudKit entitlements or a developer-owned iCloud container. iCloud Drive must be enabled and the selected folder available. Local-only folders are rejected. A revoked or stale folder grant prompts you to choose the folder again; local save files remain usable.

Each installation writes one `Peggle-Save-<random identifier>.json` in the selected folder and reads the others. Separate writers avoid overwriting simultaneous offline changes. The identifier is kept outside backups so a restored installation receives a new writer. Known provider conflict versions are merged and retained, not silently discarded. Unknown formats, malformed files, excessive file counts/data, and symbolic links stop the exchange before publication. Original game directories are never placed in iCloud Drive. User names are replaced with a neutral label in the shared JSON.

**Last checked** records a successful exchange with the locally accessible folder, not confirmation of upload or delivery to another device. iCloud Drive controls delivery and may defer it while offline. Missing downloads are requested and retried. Periodic checks run while the app is active; background execution is best effort. Manual checks show alerts. Successful launch checks are silent; immediate errors show alerts. Incoming changes during gameplay offer **Later** or **Apply and Restart**.

Changing folders disables automatic syncing and clears that connection’s pending application state. Existing game and folder save files are not deleted. A new in-game user must be explicitly linked before automatic syncing resumes. **Sync Immediately** always uses the currently selected in-game user.

## Personal builds: keep CloudKit

Setting `PEGGLE_CLOUDKIT_ENABLED=YES` selects the existing CloudKit transport, with the same container, record, linking behavior, and signed-capability checks. It does not use or bridge to the public folder transport. The two transports keep their settings, pending documents, and last-check timestamps separate. A personal build with missing CloudKit entitlements hides its cloud controls rather than switching transports silently.

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

Everyone using the same folder (public builds) or CloudKit container/account (personal builds) combines completion data. Deletions and resets do not propagate: completed entries merge as a union. This is not a full backup; use Export for original save files, scores, statistics and unfinished games.
