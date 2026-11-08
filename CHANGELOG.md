# Changelog

All notable changes to this project are documented in this file. It is generated
from Conventional Commit messages by [git-cliff](https://git-cliff.org); don't edit it by hand.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-11-07

### Features

- **domain:** Hybrid logical clock ([c43aa85](https://github.com/inner-marker/vehicle_manager_2/commit/c43aa85773d07819c7c3edb38e297da93178727c))
- **domain:** UUIDv7 record IDs ([26ffb17](https://github.com/inner-marker/vehicle_manager_2/commit/26ffb172bc144a6a5d3f1a70389786c17472b140))
- **storage:** SQLite database and migrations ([138f042](https://github.com/inner-marker/vehicle_manager_2/commit/138f0421026789fc25c72c497997ff78bb61deb6))
- **server:** Startup integrity check and health ([0b235d9](https://github.com/inner-marker/vehicle_manager_2/commit/0b235d9dd26a7b196f7f46d705ee5d498c64562b))
- **domain,server:** Command pipeline ([74cefc4](https://github.com/inner-marker/vehicle_manager_2/commit/74cefc493b90c177e886af9b25c86f8007f5811c))
- **server:** Job scheduler and nightly snapshot ([977a403](https://github.com/inner-marker/vehicle_manager_2/commit/977a403cc908240039ed6f2addc8705eb6d68a6e))
- **domain:** Users, roles, and permissions ([44844a1](https://github.com/inner-marker/vehicle_manager_2/commit/44844a1dc8714acca2d0225032446dabce4c69c1))
- **storage:** Users, grants, sessions tables ([e21f2a9](https://github.com/inner-marker/vehicle_manager_2/commit/e21f2a97c41f16a3759dfe9b7ebd37af867f3913))
- **server:** Passwords, sessions, and the signed-in user ([b8ddd60](https://github.com/inner-marker/vehicle_manager_2/commit/b8ddd607f93ec9db6a544671c41ef14dc7d9a515))
- **server:** First-Admin setup code ([7e69023](https://github.com/inner-marker/vehicle_manager_2/commit/7e69023640cb439655e03b06f857e857bf6dee9e))
- **app:** Sign-in, first-Admin setup, and mode switching ([56cbc15](https://github.com/inner-marker/vehicle_manager_2/commit/56cbc1554286c555912f2728e319781c890d5fdf))
- **app:** User management ([8e4d338](https://github.com/inner-marker/vehicle_manager_2/commit/8e4d3382c7350793d7ae7e86ad44e654d08621ed))
- Admin password recovery ([bb42f5a](https://github.com/inner-marker/vehicle_manager_2/commit/bb42f5a9f7a9d37cd3f5968dd4f28a5489dfee40))
- **app:** Display preferences ([54c3ebc](https://github.com/inner-marker/vehicle_manager_2/commit/54c3ebc3c96ee06363e7c9beb7a978f6c459f4e8))
- **domain:** Idle rules and the sign-out setting ([0142fc8](https://github.com/inner-marker/vehicle_manager_2/commit/0142fc8bd02423f978bfbb5c75d75372786172ea))
- Leave Admin mode and sign out when idle ([055ccb9](https://github.com/inner-marker/vehicle_manager_2/commit/055ccb9163e7267b19fb1d3588d8e8ae9fc21f6e))
- **domain:** Messages ([352d187](https://github.com/inner-marker/vehicle_manager_2/commit/352d187968c1c51899bf97dc410d5711fcaf9dd0))
- **server:** System messages and read/dismiss marks ([4b0ce5a](https://github.com/inner-marker/vehicle_manager_2/commit/4b0ce5a0672546574868f651bf8d0e97e6e88425))
- **app:** Message center ([4206119](https://github.com/inner-marker/vehicle_manager_2/commit/4206119dabb16c88e014e114b52c6537f6dce577))
- **app:** Refresh messages on an open screen ([ff0bb1a](https://github.com/inner-marker/vehicle_manager_2/commit/ff0bb1a42001d7837b3a331e1e16e5b19d948fe1))
- Test timers on test images ([764cba1](https://github.com/inner-marker/vehicle_manager_2/commit/764cba190db355b883c341d4bef7cecb5ed4d034))
- **domain:** Assets, groups, and grants on a group ([a51ee25](https://github.com/inner-marker/vehicle_manager_2/commit/a51ee25248fdae7a57566b2d30261c2cdfbd17b2))
- **domain:** Profiles, meters, and custom field definitions ([b8a02c0](https://github.com/inner-marker/vehicle_manager_2/commit/b8a02c017da8860939c02557700b7a67994efc1b))
- **storage:** Assets, groups, meters, and field definitions ([244b415](https://github.com/inner-marker/vehicle_manager_2/commit/244b4153b8cbb0779dc056f42cea4b9822cd3195))
- **server:** Vehicles and groups ([cae5cb3](https://github.com/inner-marker/vehicle_manager_2/commit/cae5cb3142898ea69b548daccc8c607213ac9e7f))
- **app:** Vehicles, groups in the sidebar, and adding a vehicle ([5a69407](https://github.com/inner-marker/vehicle_manager_2/commit/5a694072c66e962f38b9568bd22304c5fca2db66))
- **domain:** Stable custom field keys (decision 057) ([b728fdf](https://github.com/inner-marker/vehicle_manager_2/commit/b728fdfddb2b18e032e007200b565d360d67b0b0))
- **app:** Vehicle detail and setup ([334062b](https://github.com/inner-marker/vehicle_manager_2/commit/334062b85233ca862089e02c88a59a42d9a4d257))
- Groups and per-vehicle roles ([dc2231f](https://github.com/inner-marker/vehicle_manager_2/commit/dc2231fae34c7213611b21b683b0008348c59f70))
- **domain:** Locations and saved places ([714500f](https://github.com/inner-marker/vehicle_manager_2/commit/714500fb78bc839b1f1afef4c837e118bd2946e6))
- **geocoder:** Offline place lookup ([508c765](https://github.com/inner-marker/vehicle_manager_2/commit/508c765f13c367ef4c60838116420f29420084e3))
- **storage:** Saved places ([7dbdc30](https://github.com/inner-marker/vehicle_manager_2/commit/7dbdc306e3618d1c2da12cad85a2643d69274be8))
- **server:** Place lookup and saved places ([cbefca1](https://github.com/inner-marker/vehicle_manager_2/commit/cbefca1ac4fed4a2dd1e5376ad552cae8d8860ce))
- **app:** Places and the location picker ([df3ccc2](https://github.com/inner-marker/vehicle_manager_2/commit/df3ccc2ec0f8746164b3a4c2fb06a73faf3e9ff7))
- Nearby places in the location picker ([e6da9c9](https://github.com/inner-marker/vehicle_manager_2/commit/e6da9c90a9c94d7c0cb14560c444fcf682ff9527))
- **geocoder:** Numbered parts of a place count as districts ([2d19143](https://github.com/inner-marker/vehicle_manager_2/commit/2d1914320ea82de7e0f2d4c4d5b4ce1973435e06))
- **domain:** Dates and time zones ([a3ac984](https://github.com/inner-marker/vehicle_manager_2/commit/a3ac98407914b56c81ae7e79aa5f61267b59f9dd))
- **domain:** Quantities in the person's units ([50395bf](https://github.com/inner-marker/vehicle_manager_2/commit/50395bffffc3d4c4769e717700adaf86f74c6aa2))
- **domain:** Events, readings, and fuel-ups ([86c7146](https://github.com/inner-marker/vehicle_manager_2/commit/86c714621fcc8c76c905655ae3817fbb57bb4f67))
- Required-for on custom fields ([10477a1](https://github.com/inner-marker/vehicle_manager_2/commit/10477a18614214843c013e0f0acd25bf46d6c116))
- **storage:** Events ([6484bd2](https://github.com/inner-marker/vehicle_manager_2/commit/6484bd2e24e04763cfb5854ed2789373b2615c0c))
- **server:** Logging, finalizing, and corrections ([ae3b846](https://github.com/inner-marker/vehicle_manager_2/commit/ae3b84691fbd331416a27102413ffd16e8e4eb5c))
- **app:** + Log, fuel-ups, and readings ([7a875c7](https://github.com/inner-marker/vehicle_manager_2/commit/7a875c7e96e82608e7697af974d9e19eee837aa2))
- **app:** An event's page and corrections ([4bbd135](https://github.com/inner-marker/vehicle_manager_2/commit/4bbd13507f25a48dd59e3c23f01ceb46006ce027))
- Custom fields shown on and required for kinds of events, and typed durations ([3bcbcaa](https://github.com/inner-marker/vehicle_manager_2/commit/3bcbcaa76d88bbebf3abbf513711ce31766e5024))
- Custom fields shown on and required for kinds of events, and typed durations ([f6c8453](https://github.com/inner-marker/vehicle_manager_2/commit/f6c84530f461e28cba05f3c7a97a7c036476ed7a))
- **app:** The Timeline tab and Overview's fuel-ups ([5cbeec6](https://github.com/inner-marker/vehicle_manager_2/commit/5cbeec6c8477d2d5011b6518ca3d60252ce27c3c))
- A server instance name for the test server ([dd8150e](https://github.com/inner-marker/vehicle_manager_2/commit/dd8150e7e584a6dafa767b2a60da2c956fb3bf87))
- **domain:** Squawks ([af8984b](https://github.com/inner-marker/vehicle_manager_2/commit/af8984b95597c26515cce06261ebf93ec2c450d3))
- **domain:** Requirements, interval rules, and bundles ([4d8b466](https://github.com/inner-marker/vehicle_manager_2/commit/4d8b466fb3e4610feb4d02ae48f4e178eaa4f9de))
- **domain:** Maintenance and repair events ([f9ef2f7](https://github.com/inner-marker/vehicle_manager_2/commit/f9ef2f75c9b7179dc02c9a5e255c5d1742501254))
- **storage:** Maintenance, squawks, and requirements ([ec5e578](https://github.com/inner-marker/vehicle_manager_2/commit/ec5e5789ff052f3b0274b4dfef4a974a103bdda0))
- **server:** Maintenance, repairs, squawks, and schedules ([f3b7657](https://github.com/inner-marker/vehicle_manager_2/commit/f3b765746b4d9a07acdd906d7a092c243905bf4a))
- What the household calls squawks ([b164378](https://github.com/inner-marker/vehicle_manager_2/commit/b164378b7298d41d38952736f8c4f414b90833f6))
- **app:** Squawks ([fd4ce84](https://github.com/inner-marker/vehicle_manager_2/commit/fd4ce843d72d5d298f9069766c4ad8edc4681014))
- **app:** Requirements, bundles, and the Maintenance tab ([6d1bf86](https://github.com/inner-marker/vehicle_manager_2/commit/6d1bf8624b5e03404c348fa1b73840c64bdf25c9))
- **app:** The maintenance and repair work log ([edbd02a](https://github.com/inner-marker/vehicle_manager_2/commit/edbd02a12cf07cc8e4419b51ccb74264ebab0506))
- **app:** Maintenance on the event page and Timeline ([c1f001b](https://github.com/inner-marker/vehicle_manager_2/commit/c1f001b6cb691cb40af475de4148dd8f045b6e7d))
- **domain:** Attachments ([280dbb8](https://github.com/inner-marker/vehicle_manager_2/commit/280dbb809d17af23acd38986934e7a90d199a36c))
- **domain:** Official documents and license plates ([081b1c1](https://github.com/inner-marker/vehicle_manager_2/commit/081b1c10b1f8cf8ed6ccd549c179b85e1d50a8c2))
- **storage:** Files, attachments, documents, and plates ([0e53964](https://github.com/inner-marker/vehicle_manager_2/commit/0e5396431ea64f086bd0c48e6d3b7509957a6d67))
- **server:** Uploads and attachments ([46e4bfd](https://github.com/inner-marker/vehicle_manager_2/commit/46e4bfd23d5fbb4a0a1881687ad17c4865b94fb0))
- **server:** Official documents and license plates ([19d1a8e](https://github.com/inner-marker/vehicle_manager_2/commit/19d1a8e41fa2c244d1b02cc99ea66f05f07e9ee6))
- **server:** File checks and cleanup ([25d9d61](https://github.com/inner-marker/vehicle_manager_2/commit/25d9d61525d1164ef1b4230e6f6bdd7853fb4b67))
- **app:** Photos ([78c8e34](https://github.com/inner-marker/vehicle_manager_2/commit/78c8e3499e55c2a7927c6b31d6d837f8b092a273))
- **app:** PDFs and the viewer ([488c94f](https://github.com/inner-marker/vehicle_manager_2/commit/488c94fdc32720d42d75d36cc0414f5626b2bf86))
- **app:** The Documents tab and license plates ([e7f8cdc](https://github.com/inner-marker/vehicle_manager_2/commit/e7f8cdc9755cfc0b78ba2012a45b2695b6c708b6))
- **app:** The desktop window: title, size, and no developer menus ([ffedaeb](https://github.com/inner-marker/vehicle_manager_2/commit/ffedaeb265f73bdba1bb6c4bd8780474fbbdd0d6))
- **domain:** Lifecycle events ([5ef6209](https://github.com/inner-marker/vehicle_manager_2/commit/5ef620999d7ac55879e6b3e8ab7401f0a8cef116))
- **domain:** Reference documents, compliance modes, and applicability ([95b63bd](https://github.com/inner-marker/vehicle_manager_2/commit/95b63bd6c36e9d5b4ea6c1ea67d97224c30d43da))
- **storage:** Lifecycle events and reference documents ([592c121](https://github.com/inner-marker/vehicle_manager_2/commit/592c1213b45db756ae3bea73edf63d434a7b2fb2))
- **server:** Lifecycle events, reference documents, and the overdue return ([876c181](https://github.com/inner-marker/vehicle_manager_2/commit/876c181f096dc8faeb00cc181ee69e59029966be))
- **app:** Lifecycle events ([c8cc418](https://github.com/inner-marker/vehicle_manager_2/commit/c8cc418b44d96f81aa8a5ef4c17e917d746babd0))
- **app:** Recalls and bulletins, and compliance ([ba59eef](https://github.com/inner-marker/vehicle_manager_2/commit/ba59eef05143d2211e47516ad809881deb3536d1))
- **app:** Show each person only what they may use ([e692e33](https://github.com/inner-marker/vehicle_manager_2/commit/e692e3351fba107be8ac297ae1accb9d4b353afd))

### Fixes

- Keep one-time codes out of log files; System theme's Admin colours ([732709c](https://github.com/inner-marker/vehicle_manager_2/commit/732709c7085f192c6a1b474e65dd98137cbbcd53))
- **app:** Align the vehicle header's chips ([621de86](https://github.com/inner-marker/vehicle_manager_2/commit/621de86b8b7a531e699d8238986bce6f9501da8e))
- **app:** Pin the sidebar and top bar ([6cf4f78](https://github.com/inner-marker/vehicle_manager_2/commit/6cf4f78ca500039bfd6fabe26e1492a45329b9b7))
- **app:** Dialogs scroll on small screens ([05bf18f](https://github.com/inner-marker/vehicle_manager_2/commit/05bf18fd2c658c9eb637751903994c1639ed4ff6))
- **app:** An event's back links, and the loading page on phones ([47ca91f](https://github.com/inner-marker/vehicle_manager_2/commit/47ca91f1790f5e120f882556f5e0cde6220ece39))
- **app:** A correction deals only with the squawks still outstanding ([45ea10d](https://github.com/inner-marker/vehicle_manager_2/commit/45ea10d8663143bc3830a1c1d587378121e0ce94))
- **app:** Checkpoint feedback on the work log, squawk page, and schedule ([89fd6e0](https://github.com/inner-marker/vehicle_manager_2/commit/89fd6e0c8a393351ec0a11499da50b522715f22c))
- **app:** Checkpoint feedback on the work log, squawk page, and schedule ([cba108b](https://github.com/inner-marker/vehicle_manager_2/commit/cba108bb963c51a75f4a97d1ce79c3f642f50795))
- **app:** Maintenance test notes, and the test results ([886b35e](https://github.com/inner-marker/vehicle_manager_2/commit/886b35ebd8ef4168c674ba79b341e5f01172b1b5))
- **app:** Photos test notes, and the test results ([a0f1477](https://github.com/inner-marker/vehicle_manager_2/commit/a0f1477aaf60f18a636c049a0613cd6a14de216f))
- **app:** The lifecycle test notes ([069223a](https://github.com/inner-marker/vehicle_manager_2/commit/069223adec6790a65c3d36abbaa5ac1d65a03899))
- **app:** Only the Diagnostics list scrolls sideways on a phone ([f979616](https://github.com/inner-marker/vehicle_manager_2/commit/f979616d41c132139c824135c0de2fd27d651f03))

### Documentation

- Record Phase 2 decisions on login, display preferences, and asset kinds ([1d0aa35](https://github.com/inner-marker/vehicle_manager_2/commit/1d0aa35479d7364e4d8b781f85c461a2521247b0))
- Record the remaining Phase 2 decisions ([b7c0a1c](https://github.com/inner-marker/vehicle_manager_2/commit/b7c0a1cde20791bb627fd1d96e61ddfb094079c7))
- Phase 2 milestone draft; mobile moves to post-MVP ([f0cf87e](https://github.com/inner-marker/vehicle_manager_2/commit/f0cf87e7e16519110e12f6135ebd058d04b3cf66))
- Record field-level commands and the job system; plan Phase 2 milestone 1 ([d6eb633](https://github.com/inner-marker/vehicle_manager_2/commit/d6eb63349bcf76a276825e41453dfb9a1ee15a74))
- Phase 2 milestone 1 plan ([af8f513](https://github.com/inner-marker/vehicle_manager_2/commit/af8f513338b9dc97610c277ab5bc9d520f86fbf2))
- Database foundation test plan ([7d186cb](https://github.com/inner-marker/vehicle_manager_2/commit/7d186cb379eebe56c55c0afcb89978c4136de660))
- Record the database foundation test run ([9424f33](https://github.com/inner-marker/vehicle_manager_2/commit/9424f33f7f1648c5c71c69a0c547785a48ae3038))
- Phase 2 milestone 2 plan ([ba00a4f](https://github.com/inner-marker/vehicle_manager_2/commit/ba00a4f827573ea3d4eb1508fb75a82fb703f5c4))
- Users and login test plan ([a1c5653](https://github.com/inner-marker/vehicle_manager_2/commit/a1c5653e41aba897e4bb07bb92899dc712833838))
- Plan leaving Admin mode and signing out when idle (decision 049) ([4d5a232](https://github.com/inner-marker/vehicle_manager_2/commit/4d5a232e06d0410a130519fa8fac02e3e20fccce))
- Record the users and login test run ([91c48bf](https://github.com/inner-marker/vehicle_manager_2/commit/91c48bf80fabd01376cc0af551cfc6aa79510fd0))
- Phase 2 milestone 3 plan ([d9bd605](https://github.com/inner-marker/vehicle_manager_2/commit/d9bd605a9c04caf46b07d17dccf386e6274b71f3))
- Groups, every asset in exactly one (decision 052) ([83d8659](https://github.com/inner-marker/vehicle_manager_2/commit/83d8659fd89b46cf1ca192b887820fc3e10e80c9))
- Hold digital signatures for the records and signatures standards plan ([90d3c8a](https://github.com/inner-marker/vehicle_manager_2/commit/90d3c8a11b0ca2703fc73ca0e33b4335e3c9d12f))
- Add the signatures and records sub-plan (draft) ([30a1aba](https://github.com/inner-marker/vehicle_manager_2/commit/30a1abadcf12a566250a0b5069ff57754c8c26d4))
- Messages screen layout (decision 053) ([ea817d0](https://github.com/inner-marker/vehicle_manager_2/commit/ea817d0db096c7f0e815a38b1d40001f91e0b99c))
- Message center and idle test plan ([1b3c352](https://github.com/inner-marker/vehicle_manager_2/commit/1b3c35224ac39c3b09982254f40364bf4ccb061d))
- Add the email and voice capture sub-plans (draft) ([2ea02ea](https://github.com/inner-marker/vehicle_manager_2/commit/2ea02ead1246821cd39cf804ed28594a793d9ed4))
- Review ideas and sub-plans when planning a phase or milestone ([185995e](https://github.com/inner-marker/vehicle_manager_2/commit/185995e22bf9e283292cfaa905b9ec366f273e87))
- Number the sub-plans in the order they were added ([259f143](https://github.com/inner-marker/vehicle_manager_2/commit/259f143fa2bd407ebbe344d4b6fe368014c5f45b))
- Test plan gets the milestone's compose.yaml for test timers ([faa973e](https://github.com/inner-marker/vehicle_manager_2/commit/faa973ea02566021b8c98f57cf2272d875e7a257))
- Record the message center and idle test run ([6bc5388](https://github.com/inner-marker/vehicle_manager_2/commit/6bc5388958508eb350e8a6c7a8fc331312c73ded))
- Phase 2 milestone 4 plan ([d8d7bc3](https://github.com/inner-marker/vehicle_manager_2/commit/d8d7bc36e2200f552e93943827492f6e97fc2f0a))
- Vehicles layout (decision 056) ([11d5c52](https://github.com/inner-marker/vehicle_manager_2/commit/11d5c528f289aac0ec0f294ebaf9a508826ba2a6))
- Vehicle page layout and stable field keys (decision 057) ([4842fe5](https://github.com/inner-marker/vehicle_manager_2/commit/4842fe5f37cd78d25ad960a8593f02c97d2f4358))
- Assets and groups test plan ([c78b1c2](https://github.com/inner-marker/vehicle_manager_2/commit/c78b1c24ec883bde09c60751a233acd706189a12))
- Record the assets and groups test run ([12fa851](https://github.com/inner-marker/vehicle_manager_2/commit/12fa85195244a6387345387a65bbfdc516e1b5cf))
- Show only what each person may use (milestone 9) ([603fe89](https://github.com/inner-marker/vehicle_manager_2/commit/603fe8982fb2b0a04b1b384936c8cb9058a864b0))
- Phase 2 milestone 5 plan ([269fbb9](https://github.com/inner-marker/vehicle_manager_2/commit/269fbb99247ae93155def0b66ef398a1c4e8abe0))
- Location test plan ([25fa7fb](https://github.com/inner-marker/vehicle_manager_2/commit/25fa7fbf51d2f15def2bb51adeccce5d54a38277))
- Record the location test run ([8c02a7f](https://github.com/inner-marker/vehicle_manager_2/commit/8c02a7f34e4c2a6d71459849f9bf3dac53ab2d03))
- Phase 2 milestone 6 plan ([ca0467d](https://github.com/inner-marker/vehicle_manager_2/commit/ca0467de76da5f2cf5c07516e00c81926f2e4ba2))
- Events checkpoint test ([7d45d52](https://github.com/inner-marker/vehicle_manager_2/commit/7d45d52bc990972e36203cd11159aaf880087003))
- Events test plan, part 2 ([10e6d92](https://github.com/inner-marker/vehicle_manager_2/commit/10e6d9205daad261e00300973766b34c495f2e7a))
- Events test plan, part 2 ([9c93bd1](https://github.com/inner-marker/vehicle_manager_2/commit/9c93bd1efc2f45de783231be63e863c61aa15454))
- Record the events test runs ([6d64f3e](https://github.com/inner-marker/vehicle_manager_2/commit/6d64f3ea17342fda84da30092a915b53720b630e))
- A test server beside the main server ([b9cba43](https://github.com/inner-marker/vehicle_manager_2/commit/b9cba4300af79b8899b8b12267b709414d1374c8))
- Two servers on the NAS ([2eec547](https://github.com/inner-marker/vehicle_manager_2/commit/2eec547ed86303610e42d02ae2e35e2ceb51abe3))
- Record the test server switch-over ([44b4c1b](https://github.com/inner-marker/vehicle_manager_2/commit/44b4c1b7bb0ef1c2d7f411d5b2704d0a0946fe31))
- Phase 2 milestone 7 plan ([03973aa](https://github.com/inner-marker/vehicle_manager_2/commit/03973aa0d8cbe4663a7c94c8a88a637669bd0667))
- License plates get an issuing authority and their own history (milestone 8) ([e7bbfae](https://github.com/inner-marker/vehicle_manager_2/commit/e7bbfae8290b25d92a24a8aa9a529749f837b93a))
- Maintenance checkpoint test ([d69dc60](https://github.com/inner-marker/vehicle_manager_2/commit/d69dc608d6bf8acb83dfb7e235d06c1e94d6b937))
- Maintenance test plan, part 2 ([cd2aba4](https://github.com/inner-marker/vehicle_manager_2/commit/cd2aba46fbebe032dfcfd0b41c7f164d5b2ede19))
- Part 2 step 8 passed; milestone 7 done ([6095e13](https://github.com/inner-marker/vehicle_manager_2/commit/6095e131ad9f4027507c8a7917252759f2181a9b))
- Phase 2 milestone 8 plan ([67e6ab2](https://github.com/inner-marker/vehicle_manager_2/commit/67e6ab233c17bef26b0f38901571a270fe915fcc))
- Photo screens sketch; sharing files from other apps in the plan ([098efdf](https://github.com/inner-marker/vehicle_manager_2/commit/098efdff0444bc43b7420568b8ff802f17a85c2d))
- Photos and documents checkpoint test ([62c5e71](https://github.com/inner-marker/vehicle_manager_2/commit/62c5e7162dcd0318381ccf82029e17b69bb00a4d))
- Photos and documents test plan, part 2 ([86a5f61](https://github.com/inner-marker/vehicle_manager_2/commit/86a5f61306bd471a54c6da00dba6ccac8ef56318))
- Photos and documents test plan, part 2 ([29be0b1](https://github.com/inner-marker/vehicle_manager_2/commit/29be0b152fbd20ac89d05f69580b5a018eba4fba))
- Phase 2 milestone 9 plan ([42cf0aa](https://github.com/inner-marker/vehicle_manager_2/commit/42cf0aa5077bed2f97ca8c0f862618608894a0dd))
- Plan reviews include open GitHub issues ([ed3f69b](https://github.com/inner-marker/vehicle_manager_2/commit/ed3f69b83c55187ce6e5b4f4aa51e8644a906742))
- Lifecycle test plan ([76fca09](https://github.com/inner-marker/vehicle_manager_2/commit/76fca09052071cc0fbdfe908cb39685e015fb299))
- Record the lifecycle test run ([c4656ae](https://github.com/inner-marker/vehicle_manager_2/commit/c4656ae3d141856effbde86bebab0e9292212e38))
- The v0.2.0 release plan ([47eb4dc](https://github.com/inner-marker/vehicle_manager_2/commit/47eb4dcb1490ad1739e851ded0e144c9c79079cf))
- README ([2df66c7](https://github.com/inner-marker/vehicle_manager_2/commit/2df66c750af647aae50df16309d544dbb9ba2031))
- V0.2.0 regression checklist ([07f3d03](https://github.com/inner-marker/vehicle_manager_2/commit/07f3d03836e90ccc3a9ae9bfd54e1dbf7daf94c4))

### Tests

- **storage:** The v0.2.0 migration fixture ([742e098](https://github.com/inner-marker/vehicle_manager_2/commit/742e098c3bccb44359ed0464faff4d015ca2a6f3))

### Build

- Database and snapshots in the NAS deployment ([5fe7caa](https://github.com/inner-marker/vehicle_manager_2/commit/5fe7caaf4e2cbe1894f93f859bc8260eb0848d6a))
- Geocoder data file ([7b3d810](https://github.com/inner-marker/vehicle_manager_2/commit/7b3d8109551a4144245595560f513efe49339866))

### CI

- Dev images without test timers ([a33ade9](https://github.com/inner-marker/vehicle_manager_2/commit/a33ade90a744f2fab28f50791867c93625e178b7))

### Chores

- Record storage's serde dependencies in Cargo.lock ([ee6e493](https://github.com/inner-marker/vehicle_manager_2/commit/ee6e4935b51699c8526c2b149ef1bdfebc840ca8))

## [0.1.0] - 2026-10-29

### Features

- **domain:** Add UnixMillis, Clock trait, and shared test support (decision 3) ([7b4e111](https://github.com/inner-marker/vehicle_manager_2/commit/7b4e11161b3cbef561749b5ab80beb2f1a9c8727))
- Debug log foundation ([1af6ef6](https://github.com/inner-marker/vehicle_manager_2/commit/1af6ef6aecc1946c76b5cd2c401bc9992dfb09dc))
- Translation layer ([1394f1d](https://github.com/inner-marker/vehicle_manager_2/commit/1394f1d5942253cecc958f8e4bf0523482288644))
- **app:** Skeleton UI and navigation ([4a4e641](https://github.com/inner-marker/vehicle_manager_2/commit/4a4e6411e1553af2e404072f5d6f410871d5b4db))
- Auth middleware, handshake, and test server functions ([904fb5e](https://github.com/inner-marker/vehicle_manager_2/commit/904fb5e0b0ad33221858cc76f7aa5f506cb9f73b))
- **app:** Write server logs to the data directory ([48908d3](https://github.com/inner-marker/vehicle_manager_2/commit/48908d32caeb30ded380a99309915f92b96e3729))
- **app:** Desktop data folder and log files ([dc93924](https://github.com/inner-marker/vehicle_manager_2/commit/dc93924f0f60b7f70b3bc923b4821761e6b6e91e))
- **app:** Server address setting (desktop) ([d9a45ec](https://github.com/inner-marker/vehicle_manager_2/commit/d9a45ec7e8c08ee8df2fb9ab747f31046b36930d))

### Fixes

- **ci:** Run the PR title check on pull_request (decision 5) ([29164c6](https://github.com/inner-marker/vehicle_manager_2/commit/29164c6240f88b9fd9055874377d84c37a282a3f))
- Name the compose project so docker compose matches the NAS Project ([b5e9da3](https://github.com/inner-marker/vehicle_manager_2/commit/b5e9da39dba5f2564b4dd48a491117b578132bb1))

### Documentation

- Record Phase 1 sequencing decisions ([6619815](https://github.com/inner-marker/vehicle_manager_2/commit/661981518f30876928535582a1a249f4b8f0e6ec))
- Switch to one PR per milestone ([4def350](https://github.com/inner-marker/vehicle_manager_2/commit/4def350659e66818b1bf43afc6f6b53c4229ff46))
- Record making the repository public ([5befd78](https://github.com/inner-marker/vehicle_manager_2/commit/5befd787ed1f49b894af7383420d9afafa71dba8))
- Target linux/amd64 for the first server image ([322f3b0](https://github.com/inner-marker/vehicle_manager_2/commit/322f3b01953b6bf121d13061d1abb43b0dbc77f6))
- NAS deployment test plan ([15485a1](https://github.com/inner-marker/vehicle_manager_2/commit/15485a13abe1357f87ca343de1d52a9954b3002c))
- Record the NAS test run ([fa29f17](https://github.com/inner-marker/vehicle_manager_2/commit/fa29f17ff28ad6980240b17ca38bfc89541db3a9))
- Milestone 4 plan ([5086403](https://github.com/inner-marker/vehicle_manager_2/commit/5086403e978f4e931a95ebc1dccc959fc824cd1a))
- MacOS desktop test plan ([de723b7](https://github.com/inner-marker/vehicle_manager_2/commit/de723b79d9b158d040ab1ae322cfc310737a0ee7))
- Record the macOS test run ([867fc61](https://github.com/inner-marker/vehicle_manager_2/commit/867fc61eb482c7ddefe23c49fec076594882f337))
- Mark Phase 1 complete ([f5905ee](https://github.com/inner-marker/vehicle_manager_2/commit/f5905eeb5d49a47ae0841e1a94eb3fc06b614ca2))

### Build

- Server Docker image and compose file ([66d8c4e](https://github.com/inner-marker/vehicle_manager_2/commit/66d8c4e683e88ee0ab15d3331b0ea8a58d1b6e7b))

### CI

- Add fast checks, PR-to-main gate, and Dependabot (decision 2) ([9f6a97e](https://github.com/inner-marker/vehicle_manager_2/commit/9f6a97e595a963a4a711742e3a9644b6097da015))
- Release workflow and git-cliff ([09e5266](https://github.com/inner-marker/vehicle_manager_2/commit/09e52667e495fe77c126f31af55117a55c86e3ca))
- Publish server images to GHCR ([354aa4a](https://github.com/inner-marker/vehicle_manager_2/commit/354aa4ac8e87e1e12e67bbd5d9656ce70ee331ab))

### Chores

- Allow read-only gh pr commands for agents (decision 4) ([4588c56](https://github.com/inner-marker/vehicle_manager_2/commit/4588c56594e6553150ee11f623ccdadf7e5bd2ce))
- Release v0.1.0 ([358dced](https://github.com/inner-marker/vehicle_manager_2/commit/358dced0b3e927cb77f7a44b02b57988d92bee3e))

[0.2.0]: https://github.com/inner-marker/vehicle_manager_2/compare/v0.1.0..v0.2.0
[0.1.0]: https://github.com/inner-marker/vehicle_manager_2/tree/v0.1.0

<!-- generated by git-cliff -->
