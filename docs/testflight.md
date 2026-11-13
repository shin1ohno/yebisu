# TestFlight 配布

SH1-170 の TestFlight 経路は `.github/workflows/testflight.yml` です。`main` に含まれる `vMAJOR.MINOR.PATCH` タグの push、または `main` を選んだ `workflow_dispatch` で、Rust の release device bundle をビルドし、App Store 配布署名を付け、App Store Connect にアップロードします。タグは workspace の Cargo version と一致する必要があります。PR や任意の作業ブランチでは署名ジョブを実行しません。既存の CI workflow は変更していません。

## 所有者の準備

1. App Store Connect で **`com.shin1ohno.yebisu` を登録**し、app record の数値 Apple ID を控えます。
2. App Store Connect の **API キーを作成**します。team issuer を持ち、対象アプリへのアップロード権限があるキーを使います。
3. API キーを GitHub の **environment `testflight` に secrets として保存**します。次の署名用 secrets も同じ environment に保存します。
4. 初回アップロード後、App Store Connect で processing の結果と internal group への追加を確認します。**外部テスターには Beta App Review が必要**です。テスト情報を入力し、外部グループ用のビルドを審査に提出します。

| Environment secret | 内容 |
| --- | --- |
| `ASC_KEY_ID` | API キーの ID |
| `ASC_ISSUER_ID` | team issuer の UUID |
| `ASC_KEY_P8` | `.p8` の PEM 本文（base64 ではありません） |
| `ASC_APP_ID` | app record の数値 Apple ID |
| `APPLE_TEAM_ID` | 既存署名チーム `AL9567565U` |
| `IOS_P12_BASE64` | Apple Distribution 証明書と秘密鍵を含む `.p12` の base64 |
| `IOS_P12_PASSWORD` | `.p12` の空でないパスワード |
| `IOS_PROFILE_BASE64` | `com.shin1ohno.yebisu` 用 App Store 配布 profile の base64 |

API キーはコード署名証明書とは別です。Mac mini の開発用 profile はそのまま TestFlight 用には使いません。App Store 配布 profile は有効期限内、対象 Team/App ID と一致、`get-task-allow=false`、登録端末リストなしである必要があります。helper がこれらと証明書の一致を検査します。証明書・秘密鍵・profile は所有者が準備します。この変更は Mac mini、Apple 側の登録、secrets の設定を実行しません。

environment は `main` と承認済み release タグに限定し、必要なら reviewer を設定します。タグを作成できる人を制限してください。workflow 内でも対象 repository、ref、main への包含を検査します。手動実行時は必ず `main` を選びます。

## 実行と確認

所有者の準備と PR のマージ後に、GitHub Actions の TestFlight を `main` から手動実行できます。通常のリリースでは Cargo version を更新して CI を通し、そのコミットに一致する version タグを push します。本 PR の検証ではタグ作成・push・署名・アップロードを行いません。

macos-26 上で Xcode/SDK を記録し、repository の Rust pin と `aarch64-apple-ios` を使います。ビルドは既存の 6 GiB ディスク guard を通し、Cargo jobs は1です。専用 target を使い、署名情報や IPA を cache/artifact に保存しません。秘密情報不足はビルド前に名前だけを表示して失敗します。秘密ファイルは private directory に mode 600、directory は700で作成し、`always()` で除去します。署名 helper は一時専用 Keychain を削除し、検索リストを復元してから IPA を確定します。ホスト停止時の cleanup は保証できないため、使い捨て GitHub hosted runner のみを対象とします。

marketing version は Cargo version、`CFBundleVersion` は workflow run number です。1..9999 を許容し、10000 到達前には採番方式の変更が必要です。再実行は同じ build number なので、既に ASC が受理した run の再送は重複になります。新しい手動 run を開始してください。手動配布後も marketing version は同じまま新しい build number を付けられます。

upload のツール受理と TestFlight 配達は別です。所有者が ASC の validation/processing、internal group の配達、iPad 上のインストール・上書き起動・設定と Keychain 資格情報の保持を確認します。配布 build は `scripts/ios/icons.py` で不透明 RGB の Y アイコン（iPad 各サイズと1024px marketing）を生成し、`xcrun actool` で `Assets.car` と PNG を作り、生成された icon plist entries を bundle に統合します。package/upload は catalog・参照 PNG・plist entries の欠落を拒否します。macOS CI は実 actool の bundle と IPA を検査し、欠落の負例も確認します。AppIcon を含む完成 IPA、配布用 SDK メタデータ、privacy/encryption 宣言の ASC 受理は初回実アップロードで検証します。CI 成功だけで TestFlight 配布完了とは報告しません。[Apple の upload 手順](https://developer.apple.com/help/app-store-connect/manage-builds/upload-builds/)と[外部テスターの審査手順](https://developer.apple.com/help/app-store-connect/test-a-beta-version/invite-external-testers/)を参照してください。

## fork から再利用した部分

参照 revision は [shin1ohno/herdr-gpui@398c132a06723a0cc1494842a34666d173ec0200](https://github.com/shin1ohno/herdr-gpui/tree/398c132a06723a0cc1494842a34666d173ec0200) です。コピーした helper/tests は `PROVENANCE.tsv` に記録しています。

- `examples/ios-distribution/testflight.sh.example` の build → package → upload の段階分離と `distribution.py` の package/upload を採用しました。bundle ID と Cargo package/bin を yebisu に変更し、既定 features を使います。App Store profile 検査、一時 Keychain、秘密値を出力しない subprocess エラー処理を継承します。
- `examples/ios-distribution/github.yml.example` の macos-26、専用 target、同時実行1本、checkout の `persist-credentials: false` を採用しました。actions の pin は yebisu の既存 CI に合わせて `@v4` です。
- `docs/ios-app-plan.md` の「TestFlight from CI」から environment 名、API キー secret 名、一時ファイル除去、run number 採番を採用しました。Swift Xcode project 用の archive/export と自動 provisioning は採用していません。Rust bundle の署名は `.p12` と App Store profile を使います。
- `examples/ios-distribution/self-hosted.md` と `PREPARE.md` の既存 Mac/OTA 案は比較資料です。self-hosted runner の設定、Ad Hoc 配信、Mac mini 操作は追加していません。

GitHub Releases 向け IPA は [ipa-signing-options.md](ipa-signing-options.md) の所有者判断後に実装します。
