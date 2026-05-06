// ============ Status Dialog Component ============
// نافذة عرض حالة تفعيل ويندوز وأوفيس

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Monitor, FileText, Loader2, Copy } from "lucide-react";
import { motion } from "framer-motion";
import { toast } from "sonner";
import { LogViewer } from "@/components/ui/LogViewer";

interface StatusDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  status: { windows: string; office: string };
  statusLoading: boolean;
  statusResult: "none" | "success" | "error";
  logs: string[];
  isDark: boolean;
}

export function StatusDialog({
  open,
  onOpenChange,
  status,
  statusLoading,
  statusResult,
  logs,
  isDark,
}: StatusDialogProps) {
  const resultBorder =
    statusResult === "success"
      ? "bg-green-500/20 border border-green-500"
      : statusResult === "error"
      ? "bg-red-500/20 border border-red-500"
      : "";

  const resultTextColor =
    statusResult === "success"
      ? "text-green-600"
      : statusResult === "error"
      ? "text-red-600"
      : "";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={`rounded-xl ${
          isDark
            ? "bg-gradient-to-br from-slate-900 to-slate-800 border border-cyan-500/30"
            : "bg-white border border-slate-200"
        }`}
        onPointerDownOutside={(e) => {
          if (statusLoading) e.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle className={`text-2xl ${isDark ? "text-cyan-300" : "text-[#1E293B]"}`}>
            حالة التفعيل
          </DialogTitle>
          <DialogDescription className={isDark ? "text-cyan-200/60" : "text-slate-500"}>
            معلومات التفعيل الحالية للنظام
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {/* Windows Status */}
          <motion.div
            className={`flex items-center justify-between p-5 rounded-xl transition-colors duration-700 ${
              resultBorder ||
              (isDark
                ? "bg-blue-900/20 border border-blue-500/30"
                : "bg-[#F1F5F9] border border-[#4682B4]/20")
            }`}
          >
            <div className="flex items-center gap-3">
              <Monitor className={`w-5 h-5 ${isDark ? "text-blue-400" : "text-[#4682B4]"}`} />
              <span className={isDark ? "text-cyan-200" : "text-[#1E293B]"}>ويندوز</span>
            </div>
            {statusLoading ? (
              <div className="flex items-center gap-2">
                <Loader2 className="w-4 h-4 animate-spin text-slate-400" />
                <span className={`text-sm font-mono ${isDark ? "text-slate-400" : "text-slate-500"}`}>
                  جاري الفحص...
                </span>
              </div>
            ) : (
              <span
                className={`font-semibold ${
                  resultTextColor || (isDark ? "text-green-400" : "text-[#4682B4]")
                }`}
              >
                {status.windows}
              </span>
            )}
          </motion.div>

          {/* Office Status */}
          <motion.div
            className={`flex items-center justify-between p-5 rounded-xl transition-colors duration-700 ${
              resultBorder ||
              (isDark
                ? "bg-orange-900/20 border border-orange-500/30"
                : "bg-[#F1F5F9] border border-[#94A3B8]/20")
            }`}
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: 0.1 }}
          >
            <div className="flex items-center gap-3">
              <FileText className={`w-5 h-5 ${isDark ? "text-orange-400" : "text-[#94A3B8]"}`} />
              <span className={isDark ? "text-cyan-200" : "text-[#1E293B]"}>أوفيس</span>
            </div>
            {statusLoading ? (
              <div className="flex items-center gap-2">
                <Loader2 className="w-4 h-4 animate-spin text-slate-400" />
                <span className={`text-sm font-mono ${isDark ? "text-slate-400" : "text-slate-500"}`}>
                  جاري الفحص...
                </span>
              </div>
            ) : (
              <span
                className={`font-semibold ${
                  resultTextColor || (isDark ? "text-green-400" : "text-[#94A3B8]")
                }`}
              >
                {status.office}
              </span>
            )}
          </motion.div>

          {/* Logs */}
          <LogViewer logs={logs} isDark={isDark} maxHeightClass="max-h-40" showLabel />
        </div>

        <div className="flex gap-3 pt-4">
          <Button
            onClick={() => {
              const logText = logs.join("\n");
              navigator.clipboard.writeText(logText);
              toast.success("تم نسخ السجل");
            }}
            className={`flex-1 border ${
              isDark
                ? "bg-cyan-600/30 hover:bg-cyan-600/50 border-cyan-500/30 text-cyan-300"
                : "bg-slate-100 hover:bg-slate-200 border-slate-200 text-slate-600"
            }`}
            variant="outline"
          >
            <Copy className="w-4 h-4 ml-2" />
            نسخ السجل
          </Button>
          <Button
            onClick={() => {
              if (!statusLoading) onOpenChange(false);
            }}
            disabled={statusLoading}
            className={`flex-1 text-white ${
              isDark ? "bg-cyan-600 hover:bg-cyan-700" : "bg-[#4682B4] hover:bg-[#3b75a5]"
            } disabled:opacity-50`}
          >
            إغلاق
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
