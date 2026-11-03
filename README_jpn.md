<p align="center">
  <img src="images/EH3D_BANNER.svg" alt="EH3D-MOTIONCNC バナー" width="100%">
</p>

# 🧭 EH3D-MOTIONCNC

<p align="center"><a href="README.md">🇺🇸 English</a> | <a href="README_spa.md">🇪🇸 Español</a> | <a href="README_fra.md">🇫🇷 Français</a> | <a href="README_ita.md">🇮🇹 Italiano</a> | <a href="README_deu.md">🇩🇪 Deutsch</a> | <a href="README_zho.md">🇨🇳 中文</a> | 🇯🇵 <b>日本語</b> | <a href="README_por.md">🇵🇹 Português</a></p>

### EH3D のマシン制御プログラムの共通基盤：ジョブの表示、テーマ、8 言語のインターフェース文言

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg" alt="GPL 3.0 or later">
  <img src="https://img.shields.io/badge/Stack-Rust-DEA584.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Languages-8-00E5FF.svg" alt="Languages">
  <img src="https://img.shields.io/badge/Version-0.0.9-0f6cbd.svg" alt="Version">
  <img src="https://img.shields.io/badge/Maturity-scaffolding-FFB020.svg" alt="Maturity">
  <img src="https://img.shields.io/badge/Tests-126-0f6cbd.svg" alt="Tests">
</p>

---

**正直さの確認 - 現在実際に動くもの：**ウィンドウを持つ Rust プログラム（`motioncnc`）で、**模擬**マシンを制御します。マシンの位置（ワークとマシン）を表示し、ジョグパッドで手動移動し、ゼロを設定し、原点復帰を行い、オペレーターが入力したコマンドを受け付け、G コードのプログラムを 1 行ずつ実行（一時停止、停止、非常停止付き）し、テーブル上にプログラムの経路を描いて実行済みの行に印を付け、コントローラーが拒否したところで止まります。ウィンドウは 37 言語と 11 のテーマに対応します。コントローラーのインターフェース（EH3D-COMMON の `eh3d-controller`）には、スクリプト化した回線に対する GRBL が書かれており、他のファミリーは未対応として一覧にあります。126 件のテストがコンピューター上で動きます。**ここにあるものは実機で実行されたことがなく**、シリアルポートは GRBL のときだけ、ネットワーク接続はサーバーへプログラムを送るときだけ開き、円弧やその他のコードはシミュレーターが描画も実行もしません。

---

> **安全状態：**このプログラムは模擬マシンのみを制御し、シリアルポートもネットワーク接続も開きません。実際のコントローラーを接続する際は、動作すると言う前にまずシミュレーターで、次に実機で実証する必要があります。リミットスイッチと非常停止はマシン自身の制御ユニットの役目のままで、ウィンドウの非常停止は安全回路ではありません。

## 📋 ひと目で

| | |
|---|---|
| バージョン | `0.0.9` |
| 成熟度 | 足場 |
| テスト | 126 |
| 技術スタック | Rust |
| 実機での動作 | いいえ：模擬マシンのみを制御します |

**移動先：** [概要](#-概要) · [アーキテクチャ](#-アーキテクチャ) · [クイックスタート](#-クイックスタート) · [テスト](#-テスト) · [構成](#-リポジトリの構成) · [関連](#-関連プロジェクト)

## 🚦 プロジェクトの状況

各行はこのリポジトリの一部分で、信号は今日どこまで進んでいるかを示します。計画ではなく、リポジトリが示せるもの（テスト、ビルド、実行）に基づいて書かれています。

🟢 完了・検証済み · 🟡 動作する（既知の制限あり） · 🟠 一部のみ、または未検証 · 🔴 未実装または動作しない

| 領域 | 状態 | ここでの意味 |
|---|---|---|
| 基盤ライブラリ | 🟡 | `eh3d-view` の `scene`、`texts`、`theme` をこのプログラム自身の名前で再エクスポートする Rust クレート。隣に EH3D-COMMON のフォルダーが必要です。 |
| シーンと表示 | 🟡 | EH3D-COMMON で作成・テスト済み。このプログラムのウィンドウがテーブル上にプログラムの経路とツールの位置を描きます（コンピューター上で確認済み）。 |
| テーマと文言 | 🟡 | 11 のテーマと 37 言語のインターフェースの言葉を、このプログラムのウィンドウで確認済み。 |
| テスト | 🟡 | コンピューター上で 126 件のテストが通ります（`cargo test --features gui`）：セッション（ジョグ、ゼロ、原点復帰、プログラム、一時停止、アラーム、非常停止）、プログラムの経路、コマンドライン。模擬コントローラーは EH3D-COMMON でテストします。 |
| ウィンドウとジョグ | 🟡 | コンピューター上で模擬コントローラーと動作します：位置、ジョグパッド、ステップ、送り、原点復帰、ゼロ、ロック解除、コマンド行、ログ。実機はありません。 |
| マシン制御 | 🟠 | シミュレーターのみ：プログラムは 1 行ずつ実行され、一時停止、停止、非常停止があります。ここからはまだ実際のコントローラーに接続していません。 |
| ドキュメント | 🟡 | 8 言語の README、2 つの図、3 つのサンプルプログラム、変更履歴。 |

## 🎯 概要

* **ウィンドウ**（`src/gui.rs`、`motioncnc`）：マシンの位置（ワークとマシン）、ステップと送り付きのジョグパッド、原点復帰、ゼロ、ロック解除、非常停止、ログ付きのコマンド行、テーブル上に経路を表示するプログラム。
* **セッション**（`src/session.rs`）：`eh3d-controller` の任意のコントローラーに対してプログラムが行うこと：ジョグ、ゼロ、原点復帰、1 行の送信、プログラムの 1 行ずつの実行、一時停止、停止、アラーム。模擬コントローラーでテストします。
* **プログラムの経路**（`src/gcode.rs`）：直線移動（`G0`、`G1`、絶対または相対、mm またはインチ）を実行前に描画します。円弧は描画しません。
* **模擬コントローラー**（EH3D-COMMON の `eh3d_controller::sim`）：移動範囲を超えるとアラームを出す、メモリ上のマシン。市場のコントローラーファミリーは `eh3d-controller` に一覧があり、GRBL が最初に書かれたもので、他は未対応として表示されます。
* `eh3d-view` の**テーマと言葉**：Forge（既定）、Neon Arcade、Glass Pro、クラシック Windows グレー、Windows に追従するテーマ、そして英語、スペイン語、イタリア語、ドイツ語、フランス語、ポルトガル語、中国語、日本語のインターフェース。将来は EH3D-DRILLPRO と EH3D-ROUTEPRO のエンジンの上で、穴あけ、ルーティング、2D・3D の CNC 加工を行う 1 つのプログラムになります。その部分はまだありません。
* **メニュー、設定、ファイル**：PlanetCNC にある内容を備えた、ファイル、表示、プログラム、マシン、ヘルプのメニュー（開く、最近のファイル、保存、編集、ゼロ、保存位置、主軸、クーラント）。実行間で記憶される設定ウィンドウ（単位、言語、11 のテーマ、移動範囲、ジョグ）。スイート共通の開く・保存ウィンドウ（`eh3d-filedialog`：場所、並べ替え、フィルター、保存時の拡張子、置き換え前の確認）。
* **シリアルポートの GRBL：** `--features serial` でビルドすると、`motioncnc --grbl ポート[@ボー]` がシミュレーターの代わりに GRBL コントローラー（既定 115200 ボー）を動かし、`motioncnc --ports` がシリアルポートを一覧します。この機能なしのビルドや開けないポートはその旨を伝えて終了します。スクリプト化したコントローラーでのみ試し、実機では試していません。
* **画面に合わせるウィンドウ：** 最大化で開き、720p から 1080p、さらに大きく広い画面まで、すべてがウィンドウに合わせて拡大縮小します。
* **障害後の再開：** プログラムの実行中、到達した行を小さなファイルに保存します。ウィンドウ、コンピューター、接続のいずれかが落ちた場合、次回の起動時にその行から再開するか破棄するかを尋ねます。オペレーターが答えるまでコントローラーには何も送信されません。
* **EH3D サーバーへの送信:** 設定にサーバーのアドレス、このプログラムのトークン、マシンIDを入れると（設定ウィンドウのサーバーの項目、または設定ファイルの `server_address`、`server_token`、`server_machine`）、**プログラム > サーバーへ送信** がプログラムのテキストを G コードのジョブとして送ります。サーバーは自分のマシン（シリアルポートの GRBL コントローラー、またはシミュレーター）で実行し、ウィンドウは状態・パーセント・到達した行を上部バー、パネル、ログに表示して追います。ウィンドウの開始・一時停止・停止はサーバー上のジョブに届き、ウィンドウを閉じてもジョブは止まりません。止まったジョブはサーバーが到達した行から再開できます（次回のウィンドウ起動時に、コントローラーのプログラムと同じように提案されます）。サーバーは有効だったコードを戻し、最初の移動の前にオペレーターを待ちます。スピンドルとクーラントは `server_restore_outputs = yes` のときだけ再び始動します。代役のサーバーで試しました。サーバー自身のテストが同じリクエストをそのシミュレーターに送ります。実機では未試験です。
* **障害後の再開：** 途中で止まったプログラムは、まず元の状態に戻されます。単位と絶対距離、安全 Z（設定ファイルの `resume_safe_z`、5 mm）、主軸と回転数に達するまでの待ち時間（`resume_spindle_wait`、3 秒）、クーラント、到達していた点、モーダル状態です。復旧ファイルにはプログラムの SHA-256 が保存され、プログラムが変わっていれば再開しません。
* **ウィンドウの補助機能：**プログラム開始前の**概要**（移動回数、切削距離、設定の速度で見積もった時間、各軸の到達範囲、機械の可動範囲との照合。スイート共通の警告つき：極端に小さい、または角から遠いプログラム、テーブルより大きいプログラム、見積もりに含まれない円弧、送り速度の指定前の切削。**それでも開始**、またはプログラムが可動範囲を超える場合の拒否（`reject_out_of_travel = no` で警告に変更、`confirm_start = no` で概要を無効化））；**キーボードのキー**（Ctrl+O 開く、Ctrl+S 保存、Ctrl+E 編集、F5 開始、スペース 一時停止、Esc 停止（ウィンドウが開いているときは Esc で閉じ、テキストボックスにキーボードがあるときはスペースと Esc はボックスに任せる）、F1 で一覧表示。`shortcuts = no` で無効化）；描いた絵と名前を持つ**ツールバー**；マイナス／プラスで刻むか**直接入力**できる数値；上から**ドラッグとズーム**、または 3D で**回転とズーム**できる経路と工具；「開く」ウィンドウの **G コードファイルのプレビュー**；**クラッシュの記録**（`eh3d-crash.log`）；設定の「安全」セクション（設定ファイルの `rapid`、`summary_tiny`、`summary_far`、`summary_margin`）。
* **プログラムのキューとコントローラーのストローク：** 「開く」ウィンドウで Ctrl+クリックして選んだ複数のファイル、まとめてドロップしたファイル、またはコマンドラインで指定したファイル（`motioncnc a.nc b.nc`）は、それぞれ自分の概要を表示したあと順番に実行されます。ウィンドウには状態付きで一覧表示され、待機中のものを削除・スキップ・並べ替えでき、プログラムが完了しないとキューは停止します。GRBL コントローラーでは接続時に `$130`、`$131`、`$132`（X、Y、Z の最大ストローク）を読み取り、概要と拒否の判定にはそれと `travel_x`、`travel_y`、`travel_z` の小さい方を使います。コントローラーが応答しない、または値を返さない場合は設定を使い、ログに記録します（`read_controller_travel = no` で読み取りを無効化）。

## 🧩 EH3D での役割

EH3D-MOTIONCNC は EH3D のマシン制御プログラムで、現在は模擬マシン上で動作します。パスで EH3D-COMMON に依存し（フォルダー `EH3D-COMMON` をこのフォルダーの隣に置く必要があります：ビュー、テーマ、言葉、コントローラー）、現時点で他の EH3D リポジトリには依存しません。

## 🏛️ アーキテクチャ

<p align="center">
  <img src="images/architecture.svg" alt="EH3D-MOTIONCNC のアーキテクチャ：誰が使うか、何があるか、何に依存するか" width="100%">
</p>

各部分の組み合わせ：誰が使うのか、このリポジトリに何があるのか、何に依存するのか。図は `docs/architecture.json` から描かれ、コードと照合され、将来のものは将来として示されます。

## 🖼️ スクリーンショット

<p align="center">
  <img src="images/screenshots/program-running.png" alt="a program running" width="100%">
</p>

<p align="center"><em>模擬マシンで実行中のプログラム：実行済みの行は緑、現在の行は明るく表示され、経路がテーブル上に描かれ、ツールはマシンのある位置にあります。</em></p>

<p align="center">
  <img src="images/screenshots/spiral-neon.png" alt="Midnight theme" width="100%">
</p>

<p align="center"><em>ミッドナイトテーマと、実行が終わった長いプログラム。</em></p>

<p align="center">
  <img src="images/screenshots/square-german.png" alt="German interface, light theme" width="100%">
</p>

<p align="center"><em>明るいペーパーテーマのドイツ語インターフェース：ウィンドウのすべての言葉が言語に従います。</em></p>

<p align="center">
  <img src="images/screenshots/settings-window.png" alt="settings window" width="100%">
</p>

<p align="center"><em>設定ウィンドウ。左のセクションは PlanetCNC のツリーのようで、言語、11 のテーマ、表示内容を選べます。</em></p>

<p align="center">
  <img src="images/screenshots/open-window.png" alt="Open window" width="100%">
</p>

<p align="center"><em>スイート共通の「開く」ウィンドウ：場所、最近のファイル、日付とサイズ付きのフォルダー一覧、ファイル名、種類。</em></p>

## 🔒 セキュリティと安全

このプログラムは、GRBL 用に起動したとき（`--grbl`、`--features serial` でのビルド）だけシリアルポートを開き、オペレーターがプログラムを送るときだけ、設定にアドレスがある EH3D サーバーへネットワーク接続を開きます。トークンは設定ファイルにテキストで保存されます。オペレーターが選ぶ G コードファイル（テキストとして読み込み、自動では実行しない）以外の他人のファイルは読まず、安全システムではありません。[SECURITY.md](SECURITY.md) を参照してください。

## 🚀 クイックスタート

安定版の Rust ツールチェーンと、このフォルダーの隣の `EH3D-COMMON` フォルダーが必要です。

```bash
cargo build --release --features gui
cargo test --features gui
cargo clippy --all-targets --features gui -- -D warnings
cargo fmt --all --check
python -I tools/ci_validate.py
cargo run --release --features gui -- examples/02_frame_and_pocket.nc
```

`build.bat` / `build.sh` はプログラムをビルドし、`test.bat` / `test.sh` はテストを実行し、`run.bat` / `run.sh` はサンプルプログラムでウィンドウを開きます。`NOPAUSE` を設定しない限り、スクリプトは最後にキー入力を待ちます。

## 🧪 テスト

`cargo test --features gui` は 126 件のテストを実行します：セッション（ジョグとゼロ、プログラムの 1 行ずつの実行、一時停止と再開、拒否された行、非常停止とアラーム、コマンドのログ、原点復帰）、プログラムの経路、コマンドライン。ビューと模擬コントローラーは EH3D-COMMON でテストします。CI は `cargo fmt --check` と警告をエラーとする `cargo clippy` も実行し、`python -I tools/ci_validate.py` がマニフェスト、バージョン、変更履歴、8 つの README を確認します。

## 🏗️ ビルドとリリース

バージョンは `Cargo.toml` と `eh3d.project.json` にあり、各バージョンには [CHANGELOG.md](CHANGELOG.md) に項目があります。まだパッケージ化するプログラムがないため、ビルド済みバイナリはありません。

## 📂 リポジトリの構成

<p align="center">
  <img src="images/repository-structure.svg" alt="EH3D-MOTIONCNC のリポジトリ構成" width="100%">
</p>

<details>
<summary>ツリーのテキスト版</summary>

```text
EH3D-MOTIONCNC/
├── src/
│   ├── lib.rs                                   モジュールと、このプログラム名で再エクスポートした eh3d-view の表示
│   ├── gui.rs                                   ウィンドウ：位置、ジョグ、プログラム、経路、コマンド、ログ
│   ├── session.rs                               コントローラーで行うこと：ジョグ、ゼロ、原点復帰、プログラムの 1 行ずつの実行、一時停止、停止、アラーム
│   ├── queue.rs                                 プログラムのキュー：順序、状態、スキップ、削除、並べ替え、失敗時の停止
│   ├── gcode.rs                                 G コードプログラムの直線移動。経路の描画用
│   ├── view.rs                                  ドラッグとズームのできる平面図と、回転できる 3D ビュー（数値のみ）
│   ├── plan.rs                                  開始前のチェック：移動の時間・到達範囲・可動範囲
│   ├── cli.rs                                   コマンドライン：ファイル、言語、テーマ、画像のオプション
│   ├── bin/motioncnc.rs                         ウィンドウを開くプログラム
│   ├── server.rs                                EH3D サーバーとの接続：送信、追跡、一時停止、停止
│   ├── settings.rs                              実行間で記憶する設定
│   ├── gui/dialogs.rs                           メニュー、設定、開く、保存、編集
│   ├── gui/remote.rs                            サーバー上のプログラム：開始、一時停止、停止、再開
│   ├── gui/views.rs                             ウィンドウの中央：上から見た、または 3D の経路と工具
│   ├── gui/summary.rs                           開始前の概要ウィンドウとキー一覧
│   ├── gui/numfield.rs                          マイナス／プラスのボタンまたは直接入力で設定する数値
│   ├── gui/keys.rs                              キーボードのキーと、メニューとキー一覧が読む表
│   ├── gui/queue.rs                             ウィンドウ内のキュー：ファイルを順に実行し、それぞれ概要の後に開始
│   └── gui/icons.rs                             線で描いたボタンの絵と、ツールバーのボタン
├── examples/                                    3 つのサンプル G コード：正方形から長い渦巻きまで
├── tests/                                       G コードと設定の追加テスト
├── tools/                                       CI チェック、README チェック、図の生成ツール
├── docs/                                        アーキテクチャ図とリポジトリ構成図の元データ
├── .github/                                     CI ワークフロー、Issue と PR のテンプレート
├── images/                                      バナー、2 つの図、スクリーンショット
├── build.sh build.bat                           ライブラリをビルド
├── run.sh run.bat                               まだプログラムがないことを表示し、テストを実行
├── eh3d.project.json                            マニフェスト：バージョン、役割、成熟度、ビルドコマンド
├── Cargo.toml Cargo.lock                        Rust パッケージと固定された依存関係
├── CHANGELOG.md                                 各バージョンの変更点
├── SECURITY.md SUPPORT.md                       脆弱性の報告方法と問い合わせ先
└── CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE   貢献方法、行動規範、ライセンス
```

</details>

## 🗺️ ロードマップと既知の制限

**現在の既知の制限**

* ここにあるものは実機で実行されたことがありません。コントローラーはシミュレーター（またはシリアルポートの GRBL。試していません）で、サーバーにはテストからしか接続しません。
* シミュレーターは基本的な G コード（`G0`、`G1`、`G90`、`G91`、`G20`、`G21`、`G92`、`M3` から `M9`）のみ受け付けます。円弧、固定サイクル、プロービングは拒否され、経路は円弧を描きません。
* パスで依存しているため、隣に `EH3D-COMMON` フォルダーが必要です。

**次のステップ**

* 実際のコントローラー：まずシリアルポート経由の GRBL、次に他のファミリー、その後 HYDRA-UMC のコントローラー。
* シミュレーターと経路での円弧とその他の G コード。
* 切削の高さを含む経路の 3D 表示。
* プロービング、ツールチェンジャー、マシンの材料。
* EH3D-DRILLPRO と EH3D-ROUTEPRO のエンジンを統合し、1 つのプログラムで穴あけとルーティングを行う。

## 🌍 言語

README とテキストは、英語、スペイン語、フランス語、イタリア語、ドイツ語、中国語、日本語、ヨーロッパポルトガル語で用意されています。

## 🔗 関連プロジェクト

* [EH3D-COMMON](https://github.com/JuanenRac/EH3D-COMMON)：表示（`eh3d-view`）とジョブ ZIP（`eh3d-archive`）の置き場所
* [EH3D-DRILLPRO](https://github.com/JuanenRac/EH3D-DRILLPRO)：エンジンがここに統合される予定の穴あけプログラム
* [EH3D-ROUTEPRO](https://github.com/JuanenRac/EH3D-ROUTEPRO)：エンジンがここに統合される予定のルーティングプログラム

## 📚 ドキュメント

* [変更履歴](CHANGELOG.md)
* [セキュリティポリシー](SECURITY.md)と[サポート](SUPPORT.md)
* ジョブの表示のドキュメントは、その置き場所である EH3D-COMMON にあります。

## 🤝 コントリビュートとコミュニティ

* プルリクエストの前に `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、`python -I tools/ci_validate.py` を実行してください。すべて通る必要があります。[CONTRIBUTING.md](CONTRIBUTING.md) と[行動規範](CODE_OF_CONDUCT.md)を参照してください。
* README は 8 言語あります。文章を変えるときは 8 つのファイルすべてを変更し、2 つの図は `tools/make_architecture.py` と `tools/make_structure.py` が `docs/architecture.json` と `docs/structure.json` から描きます。
* 機械で動かしていないものを、動作するものとして書かないでください。
* 脆弱性は非公開で報告してください：[SECURITY.md](SECURITY.md)。質問、アイデア、報告：electrohobby3d@gmail.com（[SUPPORT.md](SUPPORT.md)）。

## 👤 作者

**JuanenRac (Electro Hobby 3D)** · electrohobby3d@gmail.com

## 📜 ライセンス

ソフトウェア：GPL-3.0-or-later。[LICENSE](LICENSE) を参照。スイートのハードウェア設計は CERN OHL-S を使用します。
