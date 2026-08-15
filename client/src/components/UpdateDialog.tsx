import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Download, CheckCircle2, Loader2, FolderOpen, X, Sparkles } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import type { UpdateInfo } from "@/hooks/useUpdateCheck";

interface UpdateDialogProps {
  info: UpdateInfo | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  isDark: boolean;
}

type DownloadState = "idle" | "downloading" | "done" | "error";

export function UpdateDialog({ info, open, onOpenChange, isDark }: UpdateDialogProps) {
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
      <DialogContent
        className={`rounded-xl max-w-lg ${isDark ? "bg-gradient-to-br from-slate-900 to-slate-800 border border-cyan-500/30" : "bg-white border border-slate-200"}`}
        onPointerDownOutside={(e) => {
          if (downloadState === "downloading") e.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle className={`text-2xl flex items-center gap-2 ${isDark ? "text-cyan-300" : "text-[#1E293B]"}`}>
            <Sparkles className="w-5 h-5" />
            تحديث متاح!
          </DialogTitle>
          <DialogDescription className={isDark ? "text-cyan-200/60" : "text-slate-500"}>
            <span className="inline-flex items-center gap-2 flex-wrap">
              <span className="px-2 py-0.5 rounded bg-slate-500/20 font-mono text-xs" dir="ltr">{info.current_version}</span>
              <span className={isDark ? "text-cyan-300/60" : "text-slate-400"}>←</span>
              <span className="px-2 py-0.5 rounded bg-green-500/20 font-mono text-xs" dir="ltr">{info.latest_version}</span>
            </span>
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div className={`rounded-lg border p-4 max-h-60 overflow-y-auto ${isDark ? "bg-black/40 border-cyan-500/20" : "bg-slate-50 border-slate-200"}`}>
            <p className={`text-xs mb-2 font-semibold ${isDark ? "text-cyan-300/60" : "text-slate-500"}`}>
              التغييرات:
            </p>
            <pre className={`text-sm whitespace-pre-wrap font-sans leading-relaxed ${isDark ? "text-cyan-200/80" : "text-slate-700"}`}>
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
                className={`flex items-center gap-3 p-4 rounded-lg border ${isDark ? "bg-cyan-900/30 border-cyan-500/20" : "bg-cyan-50 border-cyan-200"}`}
              >
                <Loader2 className={`w-5 h-5 animate-spin shrink-0 ${isDark ? "text-cyan-400" : "text-cyan-600"}`} />
                <div className="flex-1">
                  <p className={`text-sm font-semibold ${isDark ? "text-cyan-300" : "text-cyan-800"}`}>جاري التحميل...</p>
                  <p className={`text-xs mt-0.5 ${isDark ? "text-cyan-300/50" : "text-cyan-700/70"}`}>يتم تحميل التحديث إلى مجلد التنزيلات</p>
                </div>
              </motion.div>
            )}

            {downloadState === "done" && (
              <motion.div
                key="done"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0 }}
                className={`flex items-center gap-3 p-4 rounded-lg border ${isDark ? "bg-green-900/30 border-green-500/30" : "bg-green-50 border-green-200"}`}
              >
                <CheckCircle2 className={`w-5 h-5 shrink-0 ${isDark ? "text-green-400" : "text-green-600"}`} />
                <div className="flex-1 min-w-0">
                  <p className={`text-sm font-semibold ${isDark ? "text-green-300" : "text-green-700"}`}>تم التحميل بنجاح! ✅</p>
                  <p className={`text-xs mt-0.5 truncate ${isDark ? "text-green-300/50" : "text-green-700/70"}`} dir="ltr">{downloadPath}</p>
                </div>
              </motion.div>
            )}

            {downloadState === "error" && (
              <motion.div
                key="error"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0 }}
                className={`flex items-center gap-3 p-4 rounded-lg border ${isDark ? "bg-red-900/30 border-red-500/30" : "bg-red-50 border-red-200"}`}
              >
                <X className={`w-5 h-5 shrink-0 ${isDark ? "text-red-400" : "text-red-500"}`} />
                <div className="flex-1 min-w-0">
                  <p className={`text-sm font-semibold ${isDark ? "text-red-300" : "text-red-700"}`}>فشل التحميل</p>
                  <p className={`text-xs mt-0.5 truncate ${isDark ? "text-red-300/50" : "text-red-700/70"}`}>{errorMsg}</p>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        <div className="flex gap-3 pt-2">
          {downloadState === "idle" || downloadState === "error" ? (
            <>
              <Button
                onClick={handleDownload}
                disabled={!info.asset_url}
                className={`flex-1 text-white disabled:opacity-50 ${isDark ? "bg-cyan-600 hover:bg-cyan-700" : "bg-[#4682B4] hover:bg-[#3b75a5]"}`}
              >
                <Download className="w-4 h-4 ml-2" />
                {downloadState === "error" ? "إعادة المحاولة" : "تحميل التحديث"}
              </Button>
              <Button
                onClick={handleClose}
                className={`flex-1 border ${isDark ? "bg-slate-700 hover:bg-slate-600 text-slate-200 border-slate-500/30" : "bg-slate-100 hover:bg-slate-200 text-slate-700 border-slate-200"}`}
                variant="outline"
              >
                تذكير لاحقاً
              </Button>
            </>
          ) : downloadState === "downloading" ? (
            <Button disabled className={`flex-1 ${isDark ? "bg-slate-700 text-slate-400" : "bg-slate-100 text-slate-400"} cursor-not-allowed`}>
              <Loader2 className="w-4 h-4 ml-2 animate-spin" />
              جاري التحميل...
            </Button>
          ) : (
            <Button onClick={handleClose} className={`flex-1 text-white ${isDark ? "bg-green-600 hover:bg-green-700" : "bg-green-600 hover:bg-green-700"}`}>
              <FolderOpen className="w-4 h-4 ml-2" />
              تم — إغلاق
            </Button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
