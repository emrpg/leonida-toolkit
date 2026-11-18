export interface NativeParam {
  readonly name: string;
  readonly param_type: string;
}

export interface NativeEntry {
  readonly namespace: string;
  readonly name: string;
  readonly hash: string;
  readonly jhash?: string;
  readonly params: readonly NativeParam[];
  readonly return_type: string;
  readonly description: string;
}

export interface NativeNamespaceStat {
  readonly namespace: string;
  readonly count: number;
}

export interface NativeBookmark {
  readonly hash: string;
  readonly namespace: string;
  readonly name: string;
  readonly note?: string;
  readonly created_at?: string;
}

export type StubLang = 'cpp' | 'csharp' | 'rust';
