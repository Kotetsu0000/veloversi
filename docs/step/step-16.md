# Step 16: Release workflow と wheel 配布導線

## このステップの目的

ローカルでは Python パッケージとして install できる状態になっているが、タグ push から GitHub Release へ wheel / sdist を自動配置する導線はまだない。

Step 16 では、配布用 workflow を追加し、主要 OS / arch 向け artifact を安定して生成できる状態にする。

## このステップで行うこと

- release 専用 GitHub Actions workflow を追加する
- バージョンタグ push で workflow が起動するようにする
- GitHub Release 用 artifact を自動生成する
- wheel と sdist を GitHub Release に upload する
- Python version 検証 matrix を分離して持つ
- 将来 `ref` AI の build feature を追加しやすい workflow 構成にする

## 導入対象

- `.github/workflows/` 配下の release workflow
- 必要に応じた `README.md` の配布手順説明
- 必要に応じた build 設定の追記

## このステップの対象範囲

### 配布 matrix

- Linux
  - `x86_64`
  - `aarch64`
- macOS
  - `x86_64`
  - `arm64`
  - universal2 ではなく別 wheel とする
- Windows
  - `x86_64`

### Python version 検証 matrix

- `3.12`
- `3.13`
- `3.14`
- 将来の Python minor version を配列追記で増やせる構成

### 生成対象

- wheel
- sdist
- GitHub Release への upload

## このステップの対象外

このステップでは次を扱わない。

- PyPI publish
- `ref` AI の実装
- `ref` AI feature の有効化実装
- Windows arm 向け wheel
- Linux / macOS 以外の追加 arch

## 受け入れ条件

- [ ] release 専用 workflow が追加されている
- [ ] タグ push で起動する設定になっている
- [ ] Linux / macOS / Windows 向け wheel が生成対象に入っている
- [ ] Linux `aarch64` と macOS `arm64` が matrix に含まれている
- [ ] sdist が生成対象に入っている
- [ ] GitHub Release へ artifact を upload する構成になっている
- [ ] Python version 検証 matrix が `3.12`, `3.13`, `3.14` を持っている
- [ ] 新しい Python minor version を追記しやすい形になっている
- [ ] `make check` が成功する

## 実装開始時点の不足

現状の CI は [ci.yml](/home/kotetsu0000/program/veloversi/.github/workflows/ci.yml) のみで、Ubuntu + Python 3.12 に対する `make check` を実行している。
配布用 workflow と GitHub Release への upload 導線は未整備で、タグベースの release 運用ができない。

## 実装方針

- CI と release workflow は分離する
- 配布 matrix と Python version 検証 matrix を分けて設計する
- `abi3` 前提を活かし、配布 artifact は OS / arch 中心で構成する
- macOS は universal2 を採らず、`x86_64` と `arm64` の別 wheel を生成する
- Python minor version は検証 matrix で担保する
- version list は workflow 内で一箇所に集約し、将来追記しやすくする
- 将来 `ref` AI の build feature を追加できるよう、build コマンドは拡張しやすい形にしておく

## 段階的な進め方

### Phase 1. workflow 設計

- release trigger を決める
- 配布 matrix と検証 matrix を定義する
- artifact 名と upload 手順を決める

### Phase 2. artifact 生成

- wheel を各 OS / arch で build する
- sdist を生成する
- release artifact として収集する

### Phase 3. 検証と publish

- Python version 検証 matrix で install / import / smoke test を行う
- GitHub Release へ upload する

## 品質ゲートの扱い

- `make check` は必須とする
- workflow 自体の妥当性確認を行う
- 実タグ push までは行わない場合でも、構成として release 可能な状態にする

## 導入時の注意

- CI workflow と責務を混ぜない
- PyPI publish を入れない
- Windows arm を無理に含めない
- `ref` AI 用の feature 分岐を先回りで実装しすぎない
- runner label は、Intel 系だけ具体 label を使う
- それ以外は `latest` 系を基本にする
- ただし arm runner に `latest` 相当が無い場合は、その runner だけ具体 label を使う

## このステップを先に行う理由

今後 `ref` AI を optional build にしたい方針があるため、その前に release 導線を固めておく方が build 戦略を組み込みやすい。
また、ローカル install 可能な段階で release workflow を先に整える方が、配布観点の問題を早く表面化できる。
