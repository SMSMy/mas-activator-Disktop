import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { toast } from "sonner";

export interface UpdateInfo {
  available: boolean;
  current_version: string;
  latest_version: string;
  notes: string;
  download_url: string;
  asset_url: string;
  check_error: string | null;
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
        } else if (info.check_error) {
          toast.warning(info.check_error);
        }
      } catch {
        toast.warning("تعذر التحقق من التحديثات");
      }
    };
    check();
  }, []);

  return { updateInfo, showDialog, setShowDialog };
}
