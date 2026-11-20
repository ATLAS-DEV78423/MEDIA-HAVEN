# MEDIA_HAVEN — Consolidated Architecture and Requirements

## 1. Purpose and scope

MEDIA_HAVEN is a proposed media acquisition and playback application. It uses qBittorrent in a Docker-based isolated environment to download media into dedicated storage, presents that media through a user-facing application, prepares upcoming episodes in the background, and removes watched episodes when they are no longer needed.

This document consolidates the supplied handwritten architecture and accompanying explanation. It distinguishes intended requirements from proposed implementation decisions and unresolved questions. It is a design specification, not a record of an implemented system or a security assessment.

Use is intended for media the operator is authorized to download and distribute.

## 2. Core user experience

1. The user selects a movie or TV episode in the application.
2. The system obtains a suitable torrent link from a source that has yet to be defined.
3. qBittorrent downloads the media into a dedicated directory associated with the Docker sandbox.
4. The application discovers available media and exposes it for playback.
5. The user watches through the application's interface without interacting directly with qBittorrent.
6. While an episode plays, the system prepares the next two episodes in the background.
7. After confirmed viewing completion, the system coordinates cleanup with the downloader and removes media that is no longer needed.

The aim is a rolling two-episode buffer, controlled storage usage, and minimal interruption between episodes. Seamless playback is a goal, not a guarantee.

## 3. Architecture at a glance

### 3.1 Conceptual flow

```text
Websites / torrent links / other authorized sources
                       |
                       v
             Download orchestration
                       |
                       v
+---------------- Docker sandbox ----------------+
|                                                |
|                 qBittorrent                    |
|                       |                        |
|                       v                        |
|          Dedicated download directory          |
+-----------------------+------------------------+
                        |
              Controlled media access
                        |
                        v
           Application / playback service
                        |
                        v
                   Frontend
                        |
                        v
                      User
```

The original sketch establishes the logical flow. The orchestration and playback-service boundaries above are proposed implementation structure, not explicitly drawn components.

### 3.2 Control flow and media flow

Keep two concerns separate:

- **Control flow:** requests to add downloads, query status, change priorities, and perform cleanup.
- **Media flow:** access to video files, metadata, and playback responses.

The browser-facing frontend should consume a controlled application interface rather than receive unrestricted filesystem access or downloader credentials. How that interface is implemented remains a design decision.

## 4. Components and responsibilities

| Component | Responsibility | Status |
| --- | --- | --- |
| Frontend | Show movies and episodes, initiate playback, display progress, and report playback events | Intended |
| Application backend / orchestrator | Coordinate selection, download state, playback state, prefetching, and cleanup | Proposed implementation component |
| Source integration | Obtain torrent links or accept user-supplied links | Required capability; source mechanism unresolved |
| qBittorrent | Receive torrent links, manage transfers, and write media into the designated directory | Intended |
| Docker sandbox | Provide the intended isolation boundary for downloader activity | Intended; exact configuration unresolved |
| Dedicated media storage | Hold downloaded and prefetched media | Intended; mount and persistence model unresolved |
| Playback service / engine | Make media available to the user's player | Intended capability; implementation unresolved |
| State store | Track media identity, transfer state, playback progress, and cleanup eligibility | Proposed implementation component |
| Cleanup worker | Coordinate downloader removal and media deletion | Proposed implementation component |

These logical responsibilities do not require a separate service or container for every row.

## 5. Functional requirements

| ID | Requirement | Intended behavior |
| --- | --- | --- |
| R-01 | Torrent sources | Obtain torrent links from websites, user input, or another source integration; exact mechanism undecided |
| R-02 | Download engine | Use qBittorrent to manage downloads |
| R-03 | Downloader isolation | Run torrent downloading inside a Docker-based sandbox |
| R-04 | Dedicated storage | Write downloads only to an explicitly designated media location |
| R-05 | Controlled frontend integration | Let the application discover and access media without unrestricted access to the sandbox filesystem |
| R-06 | Playback | Allow users to watch media through the application |
| R-07 | Automatic cleanup | Remove completed-viewing episodes once they are no longer needed and cleanup is safe |
| R-08 | Predictive downloads | Prepare the next two episodes while the current episode is being watched |
| R-09 | Background operation | Manage prefetch downloads without requiring manual interaction with qBittorrent |
| R-10 | Playback continuity | Aim to have the next episode ready before the current episode ends |
| R-11 | Duplicate prevention | Do not request another download for an episode already available or actively downloading |
| R-12 | Resume protection | Preserve interrupted episodes rather than treating playback start as completion |
| R-13 | Coordinated deletion | Reconcile downloader state, file deletion, and application records |

R-01 through R-10 consolidate the supplied requirements. R-11 through R-13 make the supplied operational recommendations explicit.

## 6. Download and discovery workflow

1. Identify the selected movie or episode.
2. Resolve an appropriate torrent link through the chosen source mechanism.
3. Check whether the requested media is already available or downloading.
4. Submit the download to qBittorrent if needed.
5. Track transfer status and the intended destination.
6. Discover the downloaded files and associate them with the correct media record.
7. Mark media ready for playback according to the selected readiness policy.
8. Present availability in the frontend.

### Illustrative storage layout

```text
media/
  downloads/
    movies/
    tv/
      series-a/
        season-01/
          episode-01.mkv
          episode-02.mkv
          episode-03.mkv
```

This is an example, not a required directory structure. Naming, incomplete-download directories, subtitles, and multi-file handling remain unspecified.

## 7. Storage and playback access options

The supplied design discusses four alternatives. None has been confirmed as the final choice.

| Option | Description | Design implications |
| --- | --- | --- |
| A — Stream from sandbox | Media stays in sandbox-associated storage and a service exposes it to the player | Requires a defined media-serving interface |
| B — Shared folder / volume | Downloader writes to a dedicated mounted directory; a playback service reads the same directory | Avoids a separate copy step; permissions and cleanup ownership must be defined |
| C — Copy into application storage | Completed downloads are copied into a separate media directory for playback | Requires copy tracking and a policy for both original and copied files |
| D — Alternative | Another arrangement | Must be specified |

### Provisional recommendation from the supplied explanation

Use **Option B**, a narrowly scoped shared volume, as the working design candidate.

Suggested responsibility split:

- qBittorrent receives write access to its designated download location.
- The playback service receives read-only access where feasible.
- Cleanup is performed by a deliberately authorized backend component.
- The browser frontend accesses media through application endpoints rather than mounting storage itself.

A shared folder describes the storage relationship; streaming describes delivery to the player. A shared-volume deployment can still stream media to a browser through a playback service.

The phrase “inside Docker” describes the intended logical boundary. The final design must specify whether files live in a named volume, a bind mount, or another storage mechanism, and what survives container recreation.

## 8. Episode lifecycle and playback tracking

### Proposed lifecycle

```text
Requested -> Queued -> Downloading -> Ready -> Playing
                                            |       |
                                            |       +-> Interrupted -> Resume
                                            |       |
                                            |       +-> Watched
                                            |              |
                                            |              v
                                            |       Cleanup pending
                                            |              |
                                            +--------------v
                                                         Deleted
```

The diagram is conceptual. Download failures, retries, cancellations, and cleanup failures require additional implementation states.

### Completion policy

Playback start is not sufficient evidence that an episode has been watched. The application needs an explicit definition of completion.

Candidate signals include:

- A player-ended event.
- Progress reaching a chosen completion threshold.
- An explicit user action to mark an episode watched.

The exact threshold and treatment of seeking, credits, accidental closure, and replay remain open. Interrupted playback should preserve the file and resume position unless a separate retention policy says otherwise.

## 9. Rolling two-episode buffer

### Intended behavior

| Current episode | Desired upcoming buffer |
| --- | --- |
| Episode 1 | Episodes 2 and 3 |
| Episode 2 | Episodes 3 and 4 |
| Episode 3 | Episodes 4 and 5 |

On playback of episode N:

1. Determine the next two valid episodes in the selected ordering.
2. Check their existing download and availability states.
3. Reuse ready files and existing download tasks.
4. Enqueue only missing episodes.
5. Continue updating availability while the current episode plays.
6. Advance the buffer when the user moves to the next episode.

### Decisions still required

- Start prefetching immediately on playback or after a progress/readiness threshold?
- How should current-episode downloading compete with prefetch downloads?
- How should season boundaries and the final episode be handled?
- What happens when the user switches shows or jumps ahead?
- Should abandoned prefetch downloads pause, cancel, expire, or finish?
- What storage limit applies to prefetched media?

The target is two upcoming episodes, not an unconditional guarantee that both will be ready. The current episode, interrupted sessions, and pending cleanup can increase total retained storage.

## 10. Automatic cleanup

### Intended trigger

An episode becomes eligible for cleanup after confirmed viewing completion and after the application determines that it is no longer required.

### Proposed cleanup sequence

1. Persist the watched state.
2. Check for active playback, replay needs, and other retention requirements.
3. Identify the corresponding qBittorrent task and the files it owns.
4. Coordinate stopping or removing that task as appropriate.
5. Remove eligible media through the designated cleanup mechanism.
6. Verify or reconcile the outcome.
7. Update availability and cleanup records.
8. Retry or surface failures without falsely marking existing media as deleted.

Do not independently delete files without reconciling downloader state. The final implementation must decide whether the downloader performs file deletion or a cleanup worker does it; responsibilities should not overlap accidentally.

### Multi-episode torrents

A torrent may contain multiple episodes. Cleanup must not assume that one torrent equals one episode or remove an entire torrent's data when other episodes are still needed. Episode-to-file and file-to-torrent relationships need explicit tracking.

### Interrupted playback and replay

An interrupted episode should not be deleted merely because playback stopped. A replay grace period, manual keep option, and policy for multiple users remain optional decisions, not confirmed requirements.

## 11. Security intent and boundaries

The stated security goal is to isolate torrent-related downloading from the main application and limit access to downloaded files.

Proposed controls to evaluate during implementation:

- Restrict downloader access to explicitly required storage and interfaces.
- Keep downloader credentials out of browser code.
- Avoid unrestricted exposure of downloader management interfaces.
- Define authentication and authorization for application and media endpoints.
- Separate read-only playback access from deletion authority where practical.
- Define network access and host access policies explicitly.
- Treat downloaded media and supplied links as untrusted inputs.

These are design objectives, not verified properties of an existing deployment. Docker alone should not be treated as proof of complete isolation, and serving files from a sandbox does not by itself establish their safety.

## 12. Proposed application records

The implementation will likely need records for:

| Record | Example information |
| --- | --- |
| Media identity | Movie or series, season, episode, ordering |
| Download | Torrent identifier, status, destination, progress, errors |
| File mapping | Which files belong to which episodes and torrents |
| Playback | User or session, resume position, completion status |
| Prefetch | Requested upcoming episodes, priority, existing task references |
| Cleanup | Eligibility, pending action, outcome, retry state |

This is a conceptual data model, not a committed database schema or technology choice.

## 13. Operational cases to resolve

| Case | Expected direction / unresolved policy |
| --- | --- |
| Episode already downloaded | Reuse it rather than downloading again |
| Episode already downloading | Reuse and track the existing task |
| User stops early | Preserve resume state and avoid completion-triggered deletion |
| User switches shows | Apply a defined prefetch cancellation, pause, or retention policy |
| Next episode is unavailable | Show its actual state; do not imply readiness |
| Download fails | Record failure and define retry or source-reselection behavior |
| Cleanup fails | Keep the failure visible and reconcile state before declaring deletion complete |
| Storage is full | Apply an explicit quota, eviction, or queue-pausing policy |
| Multiple episodes share one torrent | Preserve files still needed by other episodes |
| Container or application restarts | Recover download and playback state according to the persistence model |
| Media is not directly playable | Apply a defined compatibility or transcoding policy, if supported |

Retry rules, quotas, transcoding, and multi-user behavior are not specified in the original notes.

## 14. Acceptance criteria for the intended system

1. A selected, authorized torrent can be submitted to qBittorrent through the application workflow.
2. Downloads remain confined to the configured media destination.
3. Available media appears in the frontend and can be played through the selected playback path.
4. The user does not need to operate qBittorrent directly.
5. Starting an episode schedules the next two eligible episodes according to the chosen prefetch trigger.
6. Ready or in-progress episodes do not create duplicate download tasks.
7. Interrupted playback retains the episode and its resume state.
8. Confirmed watched episodes become eligible for coordinated cleanup.
9. Cleanup does not remove files still needed by active playback or other retained episodes.
10. Application records reflect successful deletion or a recoverable failure state.
11. Frontend clients do not receive unrestricted sandbox filesystem access or downloader credentials.
12. Unavailable upcoming episodes are represented accurately rather than promising seamless playback.

These are proposed testable criteria derived from the consolidated requirements, not completed test results.

## 15. Decisions required before implementation

### Architecture and storage

- Confirm Option A, B, C, or another access model.
- Choose the backend, frontend, player, and deployment environment.
- Define container boundaries and storage persistence.
- Choose the media-serving interface and permission model.

### Acquisition and playback

- Specify how torrent links are supplied or discovered.
- Define media identity, episode ordering, and file matching.
- Decide whether playback requires a completed download or can start earlier.
- Define supported formats, subtitles, and any transcoding requirements.

### Lifecycle and scheduling

- Define watched completion precisely.
- Set replay, retention, and deletion policies.
- Define prefetch timing, priorities, and behavior on show changes.
- Specify multi-episode torrent handling.
- Decide whether multiple users or concurrent sessions are supported.

### Operations and security

- Set storage quotas and disk-full behavior.
- Define restart recovery, retries, logging, and failure visibility.
- Specify authentication, network exposure, and isolation controls.

## 16. Consolidated design statement

MEDIA_HAVEN is intended to use qBittorrent inside a Docker-based isolated environment to acquire authorized media into dedicated storage. A controlled application layer exposes media discovery and playback to a frontend without requiring users to interact with the downloader. During episode playback, an orchestrator maintains a rolling buffer of the next two episodes. After confirmed viewing completion, the system coordinates downloader state and file cleanup to limit storage accumulation. A tightly scoped shared volume is the provisional storage recommendation, but the final access model, security configuration, playback implementation, and lifecycle policies remain to be confirmed.

## 17. Source and interpretation notes

- Primary source: the supplied handwritten architecture image.
- Supporting source: the accompanying user-provided explanation and requirements breakdown.
- Illustrative directory names, state models, logical backend components, record structures, and acceptance criteria are consolidation proposals, not verbatim handwritten requirements.
- The third-party image and GitHub links embedded in the supplied explanation are not necessary to define this architecture and were not used as technical evidence in this consolidation.
