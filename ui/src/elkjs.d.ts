declare module "elkjs/lib/elk.bundled.js" {
  export interface ElkEdge {
    id: string;
    sources: string[];
    targets: string[];
  }

  export interface ElkNode {
    id: string;
    width?: number;
    height?: number;
    x?: number;
    y?: number;
    children?: ElkNode[];
    edges?: ElkEdge[];
    layoutOptions?: Record<string, string>;
  }

  export default class ELK {
    layout(graph: ElkNode): Promise<ElkNode>;
  }
}
