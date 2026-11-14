# GitHub Releases の IPA 署名方式

SH1-170 では TestFlight と GitHub Releases の IPA を配布先としています。Releases に置く IPA の署名方式は未決定です。この PR は Releases 向け IPA ジョブ、release asset 添付、タグ作成を実装しません。所有者が次の方式を選んだ後に実装します。

| 方式 | 利用者の導入手順 | 利点 | 制約・運用 |
| --- | --- | --- | --- |
| 未署名 IPA（AltStore / SideStore で再署名） | IPA を取得し、自分の Apple ID の署名で再署名して導入します | 配布者の署名秘密鍵・登録端末を Releases 経路に持ち込まずに済みます | IPA 単体では導入できません。利用者側で再署名・更新が必要です。再署名後の bundle ID、entitlements、Keychain group と既存 TestFlight 版のデータ継承を別に検証します |
| Apple Development 署名 | 所有者が指定する証明書/profile と登録端末で導入します | 所有者の開発端末での検証に使えます | 開発 profile の対象端末・有効期限・debug entitlement に依存します。一般公開配布の代用にはしません |
| Apple Distribution の Ad Hoc 署名 | 登録済み UDID を含む profile の IPA を導入します | 対象端末の利用者は自分で再署名せずに使えます | UDID の収集、端末登録枠、証明書/profile の更新が必要です。公開 asset 内の profile に端末情報が含まれるため公開範囲を決めます |

App Store 配布署名の IPA を Releases に置いても、通常の iPad に直接インストールできる Ad Hoc 配布にはなりません。TestFlight 用 IPA をそのまま公開 asset とする設計は採用していません。Enterprise 署名や第三者の共有証明書も今回の選択肢には含めません。

所有者は「未署名で利用者が再署名」、または具体的な署名方式（Development / Ad Hoc）、対象利用者と公開範囲を選んでください。署名する場合は Team、証明書/profile の管理者、更新担当を定めます。決定後は、新規インストールと既存 TestFlight 版からの切替で、設定・SSH 資格情報・接続が保持されるかを実機で確認します。現時点で保持を保証しません。

比較元は fork の [examples/ios-distribution/PREPARE.md](https://github.com/shin1ohno/herdr-gpui/blob/398c132a06723a0cc1494842a34666d173ec0200/examples/ios-distribution/PREPARE.md) と `adhoc.sh.example` です。Ad Hoc の仕組みは [Apple の配布手順](https://help.apple.com/xcode/mac/current/en.lproj/devde46df08a.html)を参照してください。
