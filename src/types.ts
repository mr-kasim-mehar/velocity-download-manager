export type DownloadStatus = "queued" | "resolving" | "downloading" | "paused" | "completed" | "failed" | "cancelled";
export type SourceType = "direct" | "media" | "torrent";

export interface DownloadItem {
  id: string;
  url: string;
  filename: string;
  destination: string;
  downloaded: number;
  total: number | null;
  speed: number;
  progress: number | null;
  peers: number | null;
  status: DownloadStatus;
  source_type: SourceType;
  media_mode: "video" | "audio";
  media_resolution: string;
  audio_format: string;
  error: string | null;
  created_at: number;
}

export interface DownloadRequest {
  url: string;
  destination?: string;
  filename?: string;
}

export interface AppSettings {
  default_destination: string;
}
