import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

export interface UpdateInfo {
  available: boolean;
  current_version: string;
  latest_version: string;
  notes: string;
  download_url: string;
  asset_url: string;
}

export function useUpdateCheck() {
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [showDialog, setShowDialog] = useState(false);

  useEffect(() => {
    const check = async () => {
      try {
        const info = await invoke<UpdateInfo>("check_update");
        if (info.available) {
          setUpdateInfo(info);
          setShowDialog(true);
        }
      } catch {
        // Silent fail - no network or no releases yet
      }
    };
    check();
  }, []);

  return { updateInfo, showDialog, setShowDialog };
}
