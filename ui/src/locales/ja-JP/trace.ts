export const traceJa = {
  observedRun: "観測済み実行",
  selectedBlockObservedRun: "選択中ブロックの観測済み実行",
  step: "ステップ",
  stepCount: "{count} ステップ",
  implementation: "実装",
  observedRoute: "観測されたルート",
  exitCode: "終了コード",
  stderr: "stderr",
  stdout: "stdout",
  evidenceOnlyHint:
    "実際に観測された実行証拠だけを表示しています。ルートIDはランタイム上の証拠であり、正規グラフの接続IDではありません。",
  unmappedOriginCount: "未対応のトレース起点 {count} 件",
  unmappedHint:
    "観測済み証拠だけを表示しています。不明な起点IDを推測で表示中ブロックへ関連付けることはありません。",
  status: {
    succeeded: "成功",
    failed: "失敗",
    notRun: "未実行",
    mixed: "混在",
  },
} as const;
