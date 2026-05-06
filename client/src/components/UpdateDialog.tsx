import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Download, CheckCircle2, Loader2, FolderOpen, X } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import type { UpdateInfo } from "@/hooks/useUpdateCheck";

interface UpdateDialogProps {
  info: UpdateInfo | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type DownloadState = "idle" | "downloading" | "done" | "error";

export function UpdateDialog({ info, open, onOpenChange }: UpdateDialogProps) {
  const [downloadState, setDownloadState] = useState<DownloadState>("idle");
  const [downloadPath, setDownloadPath] = useState("");
  const [errorMsg, setErrorMsg] = useState("");

  if (!info) return null;

  const handleDownload = async () => {
    const url = info.asset_url || "";
    if (!url) {
      toast.error("لا يوجد رابط تحميل مباشر");
      return;
    }

    setDownloadState("downloading");
    setErrorMsg("");

    try {
      // Extract filename from URL
      const urlParts = url.split("/");
      const filename = urlParts[urlParts.length - 1] || `MAS-Activator-${info.latest_version}.exe`;

      const path = await invoke<string>("download_update", { url, filename });
      setDownloadPath(path);
      setDownloadState("done");
      toast.success("تم تحميل التحديث بنجاح!");
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setErrorMsg(msg);
      setDownloadState("error");
      toast.error(`فشل التحميل: ${msg}`);
    }
  };

  const handleClose = () => {
    if (downloadState !== "downloading") {
      setDownloadState("idle");
      setDownloadPath("");
      setErrorMsg("");
      onOpenChange(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="bg-gradient-to-br from-slate-900 to-slate-800 border border-cyan-500/30 rounded-xl max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-cyan-300 text-2xl">
            تحديث متاح!
          </DialogTitle>
          <DialogDescription className="text-cyan-200/60">
            الإصدار {info.current_version} → {info.latest_version}
          </DialogDescription>
        </DialogHeader>

        <motion.div
          className="space-y-4 py-2"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
        >
          <div className="bg-black/40 border border-cyan-500/20 rounded-lg p-4 max-h-60 overflow-y-auto">
            <p className="text-cyan-300/60 text-xs mb-2 font-semibold">
              التغييرات:
            </p>
            <pre className="text-cyan-200/80 text-sm whitespace-pre-wrap font-sans">
              {info.notes}
            </pre>
          </div>

          {/* Download progress/status area */}
          <AnimatePresence mode="wait">
            {downloadState === "downloading" && (
              <motion.div
                key="downloading"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -5 }}
                className="flex items-center gap-3 p-4 rounded-lg bg-cyan-900/30 border border-cyan-500/20"
              >
                <Loader2 className="w-5 h-5 animate-spin text-cyan-400 shrink-0" />
                <div className="flex-1">
                  <p className="text-cyan-300 text-sm font-semibold">جاري التحميل...</p>
                  <p className="text-cyan-300/50 text-xs mt-0.5">يتم تحميل التحديث إلى مجلد التنزيلات</p>
                </div>
              </motion.div>
            )}

            {downloadState === "done" && (
              <motion.div
                key="done"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0 }}
                className="flex items-center gap-3 p-4 rounded-lg bg-green-900/30 border border-green-500/30"
              >
                <CheckCircle2 className="w-5 h-5 text-green-400 shrink-0" />
                <div className="flex-1 min-w-0">
                  <p className="text-green-300 text-sm font-semibold">تم التحميل بنجاح! ✅</p>
                  <p className="text-green-300/50 text-xs mt-0.5 truncate" dir="ltr">{downloadPath}</p>
                </div>
              </motion.div>
            )}

            {downloadState === "error" && (
              <motion.div
                key="error"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0 }}
                className="flex items-center gap-3 p-4 rounded-lg bg-red-900/30 border border-red-500/30"
              >
                <X className="w-5 h-5 text-red-400 shrink-0" />
                <div className="flex-1 min-w-0">
                  <p className="text-red-300 text-sm font-semibold">فشل التحميل</p>
                  <p className="text-red-300/50 text-xs mt-0.5 truncate">{errorMsg}</p>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </motion.div>

        <div className="flex gap-3 pt-2">
          {downloadState === "idle" || downloadState === "error" ? (
            <>
              <Button
                onClick={handleDownload}
                disabled={!info.asset_url}
                className="flex-1 bg-cyan-600 hover:bg-cyan-700 text-white disabled:opacity-50"
              >
                <Download className="w-4 h-4 ml-2" />
                {downloadState === "error" ? "إعادة المحاولة" : "تحميل التحديث"}
              </Button>
              <Button
                onClick={handleClose}
                className="flex-1 bg-slate-700 hover:bg-slate-600 text-slate-200 border border-slate-500/30"
                variant="outline"
              >
                تذكير لاحقاً
              </Button>
            </>
          ) : downloadState === "downloading" ? (
            <Button
              disabled
              className="flex-1 bg-slate-700 text-slate-400 cursor-not-allowed"
            >
              <Loader2 className="w-4 h-4 ml-2 animate-spin" />
              جاري التحميل...
            </Button>
          ) : (
            <>
              <Button
                onClick={handleClose}
                className="flex-1 bg-green-600 hover:bg-green-700 text-white"
              >
                <FolderOpen className="w-4 h-4 ml-2" />
                تم — إغلاق
              </Button>
            </>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
