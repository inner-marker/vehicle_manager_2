# Custom Roles Sub-Plan

*Confirmed 2026-11-27 (decision #091). Issue #27; placed in Phase 6 milestone 4 by sub-plan 04 §8 and decision #071. Built in steps 4c (server) and 4d (app).*

## 1. Goals

- A household can make roles that don't fit the built-in ones, e.g. a **Chauffeur** (fuel-ups and readings only) or an **Apprentice** (logs maintenance, but doesn't sign off squawks or change schedules).
- **Admin stays fixed** with every permission; the built-in Viewer, Driver, and Maintainer stay as they are.
- A person can hold several roles, on all vehicles, a group, or one vehicle, as now; the **mode picker** still caps what they can do right now.

## 2. What a custom role is

- **A name** (unique among roles, ignoring case, and not a built-in role's) and **ticked permissions**.
- **The permissions** are today's vehicle actions (`domain::access::AssetAction`), each with a fixed **level**:
  - **Driver level:** view the vehicle, log a fuel-up, log a reading, open a squawk.
  - **Maintainer level:** log maintenance and repairs, sign off or defer squawks, change the schedule and setup (meters, fields, requirements), manage others' records, keep a photo's original, manage documents and plates, send out for service.
  - **Admin-only** actions (the vehicle's own details, archiving and deleting it, keeping a reading that doesn't fit, recording a sale or purchase, and the household's users, groups, types, templates, and server settings) are never in a custom role.
- **Viewing** is always ticked: a role on a vehicle always lets its holder see it.

## 3. How it's checked

- **A grant** gives a person a built-in role **or** a custom role on a target (all vehicles, a group, one vehicle).
- **On a vehicle,** a person may do an action if any grant covering it allows it **and** the action's level is at or below their mode: a built-in role allows every action at or below its own level (as now); a custom role allows its ticked ones.
- **The modes a person can pick** go up to the highest level among their permissions: an Apprentice gets Driver and Maintainer; a Chauffeur, only Driver.
- **Household-wide things** that today ask for "a Maintainer or higher somewhere" (shops and labor rates, the photos setting) ask for a permission instead: managing shops and rates goes with **change the schedule and setup**.
- **The same check** runs on the server (enforcement) and on devices (what's shown, and offline on the desktop), from the same synced records.

## 4. Records and sync

- **`custom_roles`** (migration 0034): ID, name, the permissions as a JSON list, sequence, deleted. Synced to devices like the other household records.
- **Role grants** gain an optional custom role: a grant names a built-in role or a custom role, never both.
- **Removing a custom role** removes its grants (people lose what it gave them, and are told on Users); the last Admin rule is untouched, since Admin is never custom.
- **API version 4:** grants and custom roles reach the desktop, which checks them offline. The server keeps accepting version 3 (#072).

## 5. Who changes them

- The household's **Admin, in Admin mode**, on **Manage → Roles**, server only (as templates and types).
- **Everyone** sees the roles they hold and what they allow (Settings → You).

## 6. Screens

- **Manage → Roles** (sketch `docs/ui-sketches/p6m4-people-security-options.svg`, part 1): the built-in roles first (read only, their permissions shown ticked), then the household's, each with its name, its permissions in two groups (Driver level, Maintainer level), how many people hold it, and Delete.
- **Users → a person → + Add role:** the role menu lists the built-in roles, then the household's.
- **Screens show only what the person's permissions allow** (#069), as they do for roles now.

## 7. Testing

- Before any change, a test pins every built-in role's permissions in every mode as they are today; it must pass unchanged afterwards.
- A Chauffeur and an Apprentice on all vehicles, on a group, and on one vehicle, in each mode; the modes each can pick; a removed role's grants gone; offline on the desktop; an older desktop refused by the schema check.

## 8. Not now

- Custom Admin-level roles, or changing the built-in roles' permissions (the human chose fixed presets, 2026-11-27).
- Permissions on household things beyond those above (e.g. a role that manages places for others).
