export const recoveryJa = {
  recovery: "復旧",
  manualAlternatives: "手動の代替候補",
  manualRecovery: "手動復旧",
  loading: "観測済みの失敗と信頼済み代替候補を確認中…",
  unavailable: "復旧を利用できません",
  observedImpact: "観測された影響",
  failedCount: "失敗 {count}",
  affectedCount: "影響あり {count}",
  observedImpactHint:
    "失敗した実行トレースから観測した情報です。候補を選んでも正規グラフは変更されず、「適用して再計画」するまで何も実行されません。",
  route: "ルート",
  provider: "プロバイダー",
  runtime: "ランタイム",
  currentPlan: "現在の計画",
  hostDefault: "ホスト既定",
  marketplaceDisconnected:
    "マーケットプレイスのカタログはこのブリッジへ接続されていません。ホストが信頼するローカルプロバイダーだけを表示します。",
  runtimeNotExecutable:
    "選択したランタイム配置は、このローカルブリッジでは実行できません。ローカルへの暗黙フォールバックは行いません。分散ランタイム転送を接続するか、現在のローカルランタイムを選択してください。",
  noTrustedCandidate:
    "観測された失敗に対して利用できる信頼済み復旧候補がありません。",
  applyAndReplan: "適用して再計画",
  invalidPlacement: "無効な配置",
  bridgeUnavailable: "ブリッジ利用不可",
} as const;
