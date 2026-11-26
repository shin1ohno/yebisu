# TestFlight 配布

SH1-170 の TestFlight 経路は `.github/workflows/testflight.yml` です。`main` を選んだ手動実行（`workflow_dispatch`）だけで、Rust の release device bundle をビルドし、App Store 配布署名を付け、App Store Connect にアップロードします。タグの push では動きません。PR、タグ、その他のブランチでは署名ジョブを実行しません。既存の CI workflow は変更していません。

## 所有者の準備

1. App Store Connect で **`com.shin1ohno.yebisu` を登録**し、app record の数値 Apple ID を控えます。
2. App Store Connect の **API キーを作成**します。team issuer を持ち、対象アプリへのアップロード権限があるキーを使います。
3. API キーを GitHub の **environment `testflight` に secrets として保存**します。次の署名用 secrets も同じ environment に保存します。environment の設定は次の 2 つです。
   - 配置できるブランチは `main` だけにします（deployment branch policy）。
   - required reviewer に所有者（shin1ohno）を設定します。配布の run は毎回、所有者の承認を待ってから secrets を受け取ります。Claude は承認しません。
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

workflow 自身も次を検査します。

- job の条件: repository が `shin1ohno/yebisu`、イベントが `workflow_dispatch`、ref が `refs/heads/main` のときだけ job が動きます。
- `Require the origin/main tip`: `origin/main` を取り直し、checkout した HEAD が `main` の先端と一致しなければ失敗します。古い `main` のコミットからは配布しません。
- `testflight-secrets.py validate`: `GITHUB_REF` が `refs/heads/main` でなければ失敗します。

## 実行と確認

所有者の準備の後、GitHub Actions の TestFlight を `main` から手動実行します（`gh workflow run testflight.yml --ref main`）。所有者が environment `testflight` の承認をすると job が始まります。

## 段の順番と秘密ファイル

job の段は次の順に動きます。秘密ファイル（`.p12`、profile、`.p8`）がディスクにあるのは、ビルドの後から最後の削除までです。cargo の build.rs や Homebrew の formula など、ビルドと第三者のコードが動く間は、秘密ファイルはまだありません。また、それらの段には secrets を渡しません。

1. `Require the origin/main tip`: HEAD が `main` の先端と一致するかを確かめます。
2. `Validate environment secrets`（`testflight-secrets.py validate`）: 8 つの secrets の有無と形式、ref、run number、Cargo version を検査します。ファイルは書きません。brew・rustup・cargo より前に動き、不足は名前だけを表示して失敗します。
3. Xcode と SDK の記録、Rust と just の導入、`cargo fetch`、release device bundle のビルド。ビルドは使用中の Xcode の SDK メタデータを bundle の Info.plist に書き込みます（「Info.plist の SDK メタデータ」）。
4. `Write private signing and upload files`（`testflight-secrets.py materialize`）: `ASC_KEY_P8`・`IOS_P12_BASE64`・`IOS_PROFILE_BASE64` の 3 つだけを受け取り、`$RUNNER_TEMP/yebisu-testflight` を mode 700 で新しく作り、3 つのファイルを mode 600 で書きます。ディレクトリが既にあれば（ビルドが先に作った symlink を含む）失敗します。
5. `Sign and package for TestFlight`（`distribution.py package`）。
6. `Upload to App Store Connect`（`distribution.py upload`）。
7. `Remove private files`: `always()` で秘密ファイルのディレクトリを消します。

package と upload は、外部ツール（`security`、`codesign`、`ditto`、`xcrun altool`）の stdout と stderr を、成功時も失敗時も表示します。altool は `--output-format json` の JSON を出します。表示の前に、パスワード、一時 Keychain のパスワード、secrets の値、秘密ファイルのパス、`.p8` の中身を `[redacted]` に置き換えます。altool は `.p8` で JWT を署名します。JWT は key ID と issuer から作る派生値で、secrets の値とは一致しません。また、改行や `\n` で分かれると形でも見つけきれません。そのため altool の出力は、許可した項目だけを表示します。stdout の JSON からは `tool-version`・`os-version`・`success-message` と、`product-errors` の各項目の `code`・`message`・`userInfo` の `NSLocalizedDescription`・`NSLocalizedFailureReason`・`NSLocalizedRecoverySuggestion`・`underlying-errors` だけを出します。それ以外の行（JSON でない出力と stderr）は、エスケープした改行を行に分けた後、`eyJ` も `bearer`（大文字小文字を問わない）も含まず、32 文字以上の base64url のような連続（UUID を除く）も含まない行だけを出し、ほかは `[withheld: line may carry a credential]` に置き換えます。表示する文字列も、さらに上の伏せ字を通します。ツールの引数は表示しません。altool の引数は Xcode 27.2 の `xcrun altool --upload-package <file>` と API キー認証（`--api-key`・`--api-issuer`・`--p8-file-path`）で、`--apple-id` は渡しません。

本 PR の検証では、署名・アップロード・environment と secrets の作成・workflow の実行を行いません。

`xcode-27` runner 上で `DEVELOPER_DIR` により Xcode 27.2 を選び、最初の段で iphoneos SDK が 27.2 でなければ失敗します（2026-11-26 の owner 決定。image の既定は Xcode 27.1 です）。続いて Xcode/SDK を記録し、repository の Rust pin と `aarch64-apple-ios` を使います。ビルドは既存の 6 GiB ディスク guard を通し、Cargo jobs は1です。専用 target を使い、署名情報や IPA を cache/artifact に保存しません。署名 helper は一時専用 Keychain を削除し、検索リストを復元してから IPA を確定します。ホスト停止時の cleanup は保証できないため、使い捨て GitHub hosted runner のみを対象とします。ビルドの段が起動したまま残るプロセスは、同じ job の後の段で秘密ファイルを読めます。この危険は、使い捨て runner と、ビルド対象が `main` の先端だけであることで抑えています。

marketing version は Cargo version、`CFBundleVersion` は workflow run number です。1..9999 を許容し、10000 到達前には採番方式の変更が必要です。再実行は同じ build number なので、既に ASC が受理した run の再送は重複になります。新しい手動 run を開始してください。手動配布後も marketing version は同じまま新しい build number を付けられます。

upload のツール受理と TestFlight 配達は別です。所有者が ASC の validation/processing、internal group の配達、iPad 上のインストール・上書き起動・設定と Keychain 資格情報の保持を確認します。アプリアイコンは `assets/app-icon/` の D1a（light・dark・tinted の 1024px、作り方は同じフォルダの README.md）です。`scripts/ios/bundle.sh` が署名の前に `scripts/ios/icons.py` を実行し、`xcrun actool` で `Assets.car` と PNG を作り、生成された icon plist entries を bundle に統合します。開発 build と配布 build は同じアイコンを持ち、配布 build の `build` stage はアイコンの存在を確認します。package/upload は catalog・参照 PNG・plist entries の欠落を拒否します。macOS CI は実 actool の bundle と IPA を検査し、欠落の負例も確認します。AppIcon を含む完成 IPA、配布用 SDK メタデータ、privacy/encryption 宣言の ASC 受理は初回実アップロードで検証します。CI 成功だけで TestFlight 配布完了とは報告しません。[Apple の upload 手順](https://developer.apple.com/help/app-store-connect/manage-builds/upload-builds/)と[外部テスターの審査手順](https://developer.apple.com/help/app-store-connect/test-a-beta-version/invite-external-testers/)を参照してください。

## Info.plist の SDK メタデータ

`scripts/ios/bundle.sh` は、Cargo でリンクした実行ファイルに `scripts/ios/Info.plist` を写して bundle を組み立てます。Xcode でビルドした bundle には SDK と Xcode の情報を示すキーを Xcode が書き込みますが、この経路では書き込まれません。App Store Connect は、これらのキーがない bundle を次のエラーで拒否します。

- 90507: `DTPlatformName` の値がありません。
- 90534: 対応していない SDK または Xcode でビルドされています（SDK と Xcode のキーがありません）。
- 90502: 64-bit のスライスを持つのに、`UIRequiredDeviceCapabilities` に `arm64` がありません。

`scripts/ios/Info.plist` は `UIRequiredDeviceCapabilities` に `arm64` を宣言します。template にあるため、Simulator と `just ipad` の bundle にも入ります。`distribution.py build` は、使用中の Xcode から次のキーを読み取り、bundle の Info.plist に書き込みます。読み取り（probe）は Cargo のビルドより前に行い、書き込みは bundle.sh が unsigned bundle を返した後、アイコン生成の前に行います。DT* キーを書き込むのは配布 build だけです。

| キー | 値 | 取得元 |
| --- | --- | --- |
| `DTPlatformName` | `iphoneos` | 固定 |
| `DTPlatformVersion` | SDK version（例 `27.2`） | `xcrun --sdk iphoneos --show-sdk-version` |
| `DTSDKName` | `iphoneos` + SDK version（例 `iphoneos27.2`） | 同上 |
| `DTSDKBuild`、`DTPlatformBuild` | SDK build（例 `24B8118`） | `xcrun --sdk iphoneos --show-sdk-build-version` |
| `DTXcode` | 4 桁: major 2 桁 + minor + patch（Xcode 27.2 は `2720`、16.2 は `1620`） | `xcodebuild -version` の `Xcode` 行 |
| `DTXcodeBuild` | Xcode build（例 `27B52`） | `xcodebuild -version` の `Build version` 行 |
| `DTCompiler` | `com.apple.compilers.llvm.clang.1_0` | 固定 |
| `BuildMachineOSBuild` | macOS build（例 `26B5091g`） | `sw_vers -buildVersion` |

probe が失敗するか、出力の形が想定と違うと、build はどの probe かを示す固定のメッセージで止まります。probe の出力は表示しません。`package` と、`upload` が使う IPA の検査は、これらのキーのどれかが欠けた bundle、`DTPlatformName` が `iphoneos` でない bundle、`UIRequiredDeviceCapabilities` に `arm64` がない bundle を、署名と送信の前に拒否します。macOS CI は、実際の `xcrun`・`xcodebuild`・`sw_vers` で書き込んだ bundle を IPA の検査に通します。

## fork から再利用した部分

参照 revision は [shin1ohno/herdr-gpui@398c132a06723a0cc1494842a34666d173ec0200](https://github.com/shin1ohno/herdr-gpui/tree/398c132a06723a0cc1494842a34666d173ec0200) です。コピーした helper/tests は `PROVENANCE.tsv` に記録しています。

- `examples/ios-distribution/testflight.sh.example` の build → package → upload の段階分離と `distribution.py` の package/upload を採用しました。bundle ID と Cargo package/bin を yebisu に変更し、既定 features を使います。App Store profile 検査、一時 Keychain、秘密値を出力しない subprocess エラー処理を継承します。
- `examples/ios-distribution/github.yml.example` の macos-26（2026-11-26 に `xcode-27` と Xcode 27.2 へ変更）、専用 target、同時実行1本、checkout の `persist-credentials: false` を採用しました。この署名 job の checkout は `actions/checkout` の v4.3.1 commit `34e114876b0b11c390a56381ad16ebd13914f8d5` に固定しています。
- `docs/ios-app-plan.md` の「TestFlight from CI」から environment 名、API キー secret 名、一時ファイル除去、run number 採番を採用しました。Swift Xcode project 用の archive/export と自動 provisioning は採用していません。Rust bundle の署名は `.p12` と App Store profile を使います。
- `examples/ios-distribution/self-hosted.md` と `PREPARE.md` の既存 Mac/OTA 案は比較資料です。self-hosted runner の設定、Ad Hoc 配信、Mac mini 操作は追加していません。

GitHub Releases 向け IPA は [ipa-signing-options.md](ipa-signing-options.md) の所有者判断後に実装します。
