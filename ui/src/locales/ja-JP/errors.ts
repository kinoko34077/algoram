export const errorsJa = {
  validation: {
    unsupportedSchemaVersion:
      "未対応の schema_version '{actual}' です。'{expected}' が必要です。",
    duplicateSourceArtifactId:
      "ソース成果物ID '{artifactId}' が重複しています。",
    duplicateBlockId: "ブロックID '{blockId}' が重複しています。",
    duplicatePortId:
      "ブロック '{blockId}' 内でポートID '{portId}' が重複しています。",
    invalidSourceAnchorRange:
      "ブロック '{blockId}' のソースアンカーのバイト範囲 {start}..{end} が無効です。",
    missingSourceArtifact:
      "ブロック '{blockId}' が存在しないソース成果物 '{artifactId}' を参照しています。",
    duplicateConnectionId: "接続ID '{connectionId}' が重複しています。",
    missingSourceBlock:
      "接続 '{connectionId}' が存在しない送信元ブロック '{blockId}' を参照しています。",
    missingTargetBlock:
      "接続 '{connectionId}' が存在しない接続先ブロック '{blockId}' を参照しています。",
    missingSourcePort:
      "接続 '{connectionId}' が存在しない送信元ポート '{blockId}.{portId}' を参照しています。",
    missingTargetPort:
      "接続 '{connectionId}' が存在しない接続先ポート '{blockId}.{portId}' を参照しています。",
    directionMismatch:
      "接続 '{connectionId}' は out → in である必要がありますが、{sourceDirection} → {targetDirection} になっています。",
    channelMismatch:
      "接続 '{connectionId}' のチャンネルが一致しません: {sourceChannel} → {targetChannel}。",
  },

  authoring: {
    blockNoLongerExists: "ブロック '{blockId}' はすでに存在しません。",
    blockHasIncidentConnection:
      "'{blockLabel}' には接続 '{connectionId}' が付いているため削除できません。先に接続を削除してください。",
    connectionMissingSourceBlock:
      "接続 '{connectionId}' が存在しない送信元ブロック '{blockId}' を参照しています。",
    connectionMissingTargetBlock:
      "接続 '{connectionId}' が存在しない接続先ブロック '{blockId}' を参照しています。",
    connectionMissingSourcePort:
      "接続 '{connectionId}' が存在しない送信元ポート '{blockId}.{portId}' を参照しています。",
    connectionMissingTargetPort:
      "接続 '{connectionId}' が存在しない接続先ポート '{blockId}.{portId}' を参照しています。",
    directionMismatch:
      "接続 '{connectionId}' は out → in である必要がありますが、{sourceDirection} → {targetDirection} になっています。",
    channelMismatch:
      "接続 '{connectionId}' のチャンネルが一致しません: {sourceChannel} → {targetChannel}。",
    connectionNoLongerExists: "接続 '{connectionId}' はすでに存在しません。",
  },

  graphImport: {
    invalidJson: "無効なグラフJSONです。",
    invalidJsonWithError: "無効なグラフJSONです: {error}",
    mustContainObject: "グラフJSONには1つのオブジェクトが必要です。",
    missingSchemaVersion: "グラフJSONに schema_version がありません。",
    missingId: "グラフJSONに id がありません。",
    missingBlocks: "グラフJSONに blocks がありません。",
    missingConnections: "グラフJSONに connections がありません。",
  },

  persistence: {
    indexedDbRequestFailed: "IndexedDB リクエストに失敗しました。",
    indexedDbTransactionAborted: "IndexedDB トランザクションが中断されました。",
    indexedDbTransactionFailed: "IndexedDB トランザクションに失敗しました。",
    indexedDbUnavailable: "このブラウザでは IndexedDB を利用できません。",
    editorStorageOpenFailed: "エディター保存領域を開けませんでした。",
  },

  runtimeBridge: {
    invalidUrl: "ブリッジURLには有効な http(s) URL を指定してください。",
    httpOnly: "ブリッジURLには http または https を使用してください。",
    loopbackOnly:
      "ランタイムブリッジURLは localhost のループバック接続である必要があります。",
    noCredentialsQueryFragment:
      "ブリッジURLに認証情報、クエリ、フラグメントを含めることはできません。",
    originOnly: "ブリッジURLにはパスを含めず、originだけを指定してください。",
    returnedHttp: "ランタイムブリッジが HTTP {status} を返しました。",
    requestFailedHttp:
      "ランタイムブリッジへのリクエストが HTTP {status} で失敗しました。",
    tokenRequired: "ランタイムブリッジのBearerトークンを入力してください。",
    unavailable: "ランタイムブリッジを利用できません。",
    unavailableWithError: "ランタイムブリッジを利用できません: {error}",
  },

  traceProjection: {
    graphMismatch:
      "実行トレースはグラフ '{traceGraphId}' を参照していますが、現在のグラフは '{currentGraphId}' です。",
  },
} as const;
