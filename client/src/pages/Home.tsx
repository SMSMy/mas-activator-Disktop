import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Loader2, Zap, Monitor, FileText, CheckCircle2, Sun, Moon, PartyPopper } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import type { TargetAndTransition } from "framer-motion";
import { useState, useEffect, useRef, useCallback } from "react";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { useTheme } from "@/contexts/ThemeContext";
import { Confetti } from "@/components/ui/Confetti";
import { LogViewer } from "@/components/ui/LogViewer";
import { StatusDialog } from "@/components/ui/StatusDialog";

export default function Home() {
  const { theme, toggleTheme } = useTheme();
  const [isLoading, setIsLoading] = useState(false);
  const [activeAction, setActiveAction] = useState<string | null>(null);
  const [statusDialog, setStatusDialog] = useState(false);
  const [status, setStatus] = useState({ windows: "جار الفحص...", office: "جار الفحص..." });
  const [statusLoading, setStatusLoading] = useState(false);
  const [statusResult, setStatusResult] = useState<"none" | "success" | "error">("none");
  const [logs, setLogs] = useState<string[]>([]);
  const [showLogs, setShowLogs] = useState(false);
  const [celebratingKey, setCelebratingKey] = useState<string | null>(null);
  const [confettiOrigin, setConfettiOrigin] = useState<{ x: number; y: number } | null>(null);
  const buttonRefs = useRef<Record<string, HTMLDivElement | null>>({});

  const addLog = (message: string) => {
    setLogs((prev) => [...prev, `[${new Date().toLocaleTimeString()}] ${message}`]);
  };

  useEffect(() => {
    if (statusResult !== "none") {
      const timer = setTimeout(() => setStatusResult("none"), 5000);
      return () => clearTimeout(timer);
    }
  }, [statusResult]);

  const triggerCelebration = useCallback((key: string) => {
    const el = buttonRefs.current[key];
    if (el) {
      const rect = el.getBoundingClientRect();
      setConfettiOrigin({ x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 });
    }
    setCelebratingKey(key);
    setTimeout(() => setCelebratingKey(null), 2500);
  }, []);

  const handleActivation = async (action: string, command: string, buttonKey: string) => {
    setIsLoading(true);
    setActiveAction(action);
    setLogs([]);
    addLog(`جاري تنفيذ: ${action}`);

    try {
      const result = await invoke<string>(command);
      addLog(`✅ ${result}`);
      toast.success(`تم ${action} بنجاح!`);
      triggerCelebration(buttonKey);
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      addLog(`❌ ${errorMsg}`);
      toast.error(`حدث خطأ: ${errorMsg}`);
    } finally {
      setIsLoading(false);
      setActiveAction(null);
    }
  };

  const handleCheckStatus = async () => {
    setStatusDialog(true);
    setStatusLoading(true);
    setStatusResult("none");
    setStatus({ windows: "جار الفحص...", office: "جار الفحص..." });
    setLogs([]);
    addLog("🔍 جاري فحص حالة التفعيل...");

    try {
      const result = await invoke<string>("check_status");

      try {
        const parsed = JSON.parse(result);
        const winStatus = parsed.windows || "غير معروف";
        const officeStatus = parsed.office || "غير معروف";
        const officeName = parsed.office_name || "";

        setStatus({
          windows: winStatus,
          office: officeStatus,
        });

        addLog(`✅ اكتمل الفحص بنجاح`);
        addLog(`💻 ويندوز: ${winStatus}`);
        addLog(`📄 أوفيس: ${officeStatus}${officeName ? ` (${officeName})` : ""}`);

        if (parsed.error) {
          addLog(`⚠️ ملاحظة: ${parsed.error}`);
        }
      } catch {
        setStatus({ windows: result, office: "غير معروف" });
        addLog(`⚠️ تم الفحص لكن النتيجة غير مُنسقة`);
      }
      setStatusResult("success");
      triggerCelebration("status");
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      addLog(`❌ فشل الفحص: ${errorMsg}`);
      toast.error("تعذّر فحص الحالة");
      setStatus({ windows: "خطأ في الفحص", office: "خطأ في الفحص" });
      setStatusResult("error");
    } finally {
      setStatusLoading(false);
    }
  };

  const isDark = theme === "dark";

  const containerVariants = {
    hidden: { opacity: 0 },
    visible: {
      opacity: 1,
      transition: { staggerChildren: 0.1, delayChildren: 0.2 },
    },
  };

  const itemVariants = {
    hidden: { opacity: 0, y: 20 },
    visible: { opacity: 1, y: 0 },
  };

  // إصلاح خطأ TS2322: تحديد النوع صراحة باستخدام satisfies
  const cardHover = {
    scale: 1.03,
    y: -4,
    transition: { type: "spring" as const, stiffness: 400, damping: 20 },
  } satisfies TargetAndTransition;

  const cardTap = { scale: 0.97 };

  const cardStyles = {
    windows: isDark
      ? "bg-gradient-to-br from-blue-600 to-blue-800 border-blue-400/30 hover:from-blue-500 hover:to-blue-700"
      : "bg-[#4682B4] hover:bg-[#3b6c9e]",
    office: isDark
      ? "bg-gradient-to-br from-orange-600 to-red-800 border-orange-400/30 hover:from-orange-500 hover:to-red-700"
      : "bg-[#94A3B8] hover:bg-[#7e8ea3]",
    all: isDark
      ? "bg-gradient-to-br from-green-600 to-emerald-800 border-green-400/30 hover:from-green-500 hover:to-emerald-700"
      : "bg-[#475569] hover:bg-[#374151]",
    status: isDark
      ? "bg-gradient-to-br from-purple-600 to-pink-800 border-purple-400/30 hover:from-purple-500 hover:to-pink-700"
      : "bg-[#0F172A] hover:bg-[#1e293b]",
  };

  return (
    <div className="min-h-screen grid-bg-animated relative overflow-hidden" dir="rtl">
      {/* Top bar: theme toggle (left) + credits (right) */}
      <div className="absolute top-4 left-4 right-4 z-50 flex items-center justify-between">
        <button
          onClick={toggleTheme}
          className="p-2.5 rounded-full bg-card border border-border hover:border-ring transition-all duration-300 cursor-pointer"
          title={isDark ? "الوضع النهاري" : "الوضع الليلي"}
        >
          {isDark ? (
            <Sun className="w-5 h-5 text-yellow-400" />
          ) : (
            <Moon className="w-5 h-5 text-slate-600" />
          )}
        </button>
        <span className={`text-xs tracking-wide ${isDark ? "text-cyan-300/40" : "text-slate-400"}`}>
          تطوير: يزيد يحيى | الإصدار 2.1
        </span>
      </div>

      {/* Main content */}
      <motion.div
        className="relative z-10 min-h-screen flex flex-col items-center justify-center px-4 py-12"
        variants={containerVariants}
        initial="hidden"
        animate="visible"
        transition={{ staggerChildren: 0.1, delayChildren: 0.2 }}
      >
        {/* Header */}
        <motion.div className="text-center mb-16" variants={itemVariants}>
          <div className="mb-4 inline-flex items-center gap-3">
            <Monitor className={`w-10 h-10 ${isDark ? "text-cyan-400" : "text-[#4682B4]"}`} />
          </div>
          <motion.h1
            className={`text-5xl md:text-7xl font-black mb-4 ${
              isDark
                ? "bg-gradient-to-r from-cyan-400 via-blue-400 to-purple-400 bg-clip-text text-transparent"
                : "text-[#1E293B]"
            }`}
          >
            M A S
          </motion.h1>
          <motion.p className={`text-lg font-light tracking-widest ${isDark ? "text-cyan-300/80" : "text-[#64748B]"}`}>
            أداة التفعيل الشاملة
          </motion.p>
          <motion.p className={`text-xs mt-1 tracking-wide ${isDark ? "text-cyan-300/40" : "text-[#94A3B8]"}`}>
            Microsoft Activation Scripts
          </motion.p>
        </motion.div>

        {/* Main buttons grid */}
        <motion.div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-12 max-w-2xl w-full" variants={itemVariants}>
          {(["windows", "office", "all", "status"] as const).map((key) => {
            const config = {
              windows: { label: "تفعيل ويندوز", sub: "HWID Activation", icon: Monitor, cmd: "activate_windows", name: "تفعيل ويندوز" },
              office: { label: "تفعيل أوفيس", sub: "Ohook Activation", icon: FileText, cmd: "activate_office", name: "تفعيل أوفيس" },
              all: { label: "تفعيل الكل", sub: "Windows + Office", icon: Zap, cmd: "activate_all", name: "تفعيل الكل" },
              status: { label: "فحص الحالة", sub: "Status Check", icon: CheckCircle2, cmd: "", name: "" },
            }[key];

            const isThisLoading = isLoading && (key === "status" ? true : activeAction === config.name);
            const isStatus = key === "status";
            const isCelebrating = celebratingKey === key;

            return (
              <motion.div
                key={key}
                ref={(el) => { buttonRefs.current[key] = el; }}
                whileHover={cardHover}
                whileTap={cardTap}
                animate={isCelebrating ? {
                  scale: [1, 1.08, 1.03, 1.06, 1],
                  transition: { duration: 0.6, ease: "easeOut" }
                } : {}}
              >
                <Button
                  onClick={() => isStatus ? handleCheckStatus() : handleActivation(config.name, config.cmd, key)}
                  disabled={isLoading}
                  className={`w-full h-32 rounded-xl text-lg font-bold flex flex-col items-center justify-center gap-3 transition-all duration-300 shadow-lg ${cardStyles[key]} text-white relative overflow-hidden ${
                    isCelebrating ? 'ring-4 ring-green-400/70 shadow-[0_0_30px_rgba(74,222,128,0.4)]' : ''
                  }`}
                >
                  {/* Success shimmer overlay */}
                  {isCelebrating && (
                    <motion.div
                      className="absolute inset-0 bg-gradient-to-r from-transparent via-white/25 to-transparent"
                      initial={{ x: '-100%' }}
                      animate={{ x: '200%' }}
                      transition={{ duration: 0.8, ease: 'easeInOut' }}
                    />
                  )}
                  <AnimatePresence mode="wait">
                    {isThisLoading ? (
                      <motion.div
                        key="loader"
                        initial={{ opacity: 0, scale: 0.5 }}
                        animate={{ opacity: 1, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.5 }}
                      >
                        <Loader2 className="w-8 h-8 animate-spin text-white" />
                      </motion.div>
                    ) : isCelebrating ? (
                      <motion.div
                        key="celebrate"
                        initial={{ opacity: 0, scale: 0, rotate: -180 }}
                        animate={{ opacity: 1, scale: 1, rotate: 0 }}
                        exit={{ opacity: 0, scale: 0.5 }}
                        transition={{ type: 'spring', stiffness: 300, damping: 15 }}
                      >
                        <PartyPopper className="w-8 h-8 text-yellow-300 drop-shadow-[0_0_8px_rgba(253,224,71,0.6)]" />
                      </motion.div>
                    ) : (
                      <motion.div
                        key="icon"
                        initial={{ opacity: 0, scale: 0.5 }}
                        animate={{ opacity: 1, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.5 }}
                      >
                        <config.icon className="w-8 h-8 text-white" />
                      </motion.div>
                    )}
                  </AnimatePresence>
                  <span className="text-white relative z-10">{isCelebrating ? '✅ تم بنجاح!' : config.label}</span>
                  <span className="text-xs text-white/60 relative z-10">{config.sub}</span>
                </Button>
              </motion.div>
            );
          })}
        </motion.div>

        {/* Advanced Options */}
        <motion.div className="w-full max-w-2xl" variants={itemVariants}>
          <div className="text-center mb-6">
            <p className={`text-sm tracking-widest ${isDark ? "text-cyan-300/60" : "text-slate-400"}`}>
              خيارات متقدمة
            </p>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Tooltip>
              <TooltipTrigger asChild>
                <motion.div whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.97 }}>
                  <Button
                    onClick={() => handleActivation("TSforge", "activate_tsforge", "tsforge")}
                    disabled={isLoading}
                    className={`w-full rounded-lg py-6 font-semibold transition-all duration-300 ${
                      isDark
                        ? "bg-gradient-to-br from-violet-600 to-indigo-800 hover:from-violet-500 hover:to-indigo-700 text-white border border-violet-400/30"
                        : "bg-card text-[#1E293B] border border-slate-200 hover:border-violet-300 hover:bg-violet-50"
                    }`}
                  >
                    {isLoading && activeAction === "TSforge" ? (
                      <Loader2 className="w-5 h-5 animate-spin ml-2" />
                    ) : null}
                    TSforge (الكل)
                  </Button>
                </motion.div>
              </TooltipTrigger>
              <TooltipContent side="bottom" className={`text-sm max-w-xs text-center ${isDark ? "bg-slate-800 border border-violet-500/30 text-cyan-200" : "bg-white border border-slate-200 text-slate-700"}`}>
                تفعيل ويندوز وأوفيس عبر TSforge - يدعم إصدارات Windows 10/11 حتى 2026 وإصدارات Server
              </TooltipContent>
            </Tooltip>

            <Tooltip>
              <TooltipTrigger asChild>
                <motion.div whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.97 }}>
                  <Button
                    onClick={() => handleActivation("Online KMS", "activate_kms", "onlinekms")}
                    disabled={isLoading}
                    className={`w-full rounded-lg py-6 font-semibold transition-all duration-300 ${
                      isDark
                        ? "bg-gradient-to-br from-cyan-600 to-teal-800 hover:from-cyan-500 hover:to-teal-700 text-white border border-cyan-400/30"
                        : "bg-card text-[#1E293B] border border-slate-200 hover:border-cyan-300 hover:bg-cyan-50"
                    }`}
                  >
                    {isLoading && activeAction === "Online KMS" ? (
                      <Loader2 className="w-5 h-5 animate-spin ml-2" />
                    ) : null}
                    Online KMS
                  </Button>
                </motion.div>
              </TooltipTrigger>
              <TooltipContent side="bottom" className={`text-sm max-w-xs text-center ${isDark ? "bg-slate-800 border border-cyan-500/30 text-cyan-200" : "bg-white border border-slate-200 text-slate-700"}`}>
                تفعيل أوفيس عبر KMS - يحتاج اتصال بالإنترنت، التفعيل يستمر 180 يوم ويتم التجديد تلقائياً
              </TooltipContent>
            </Tooltip>
          </div>
        </motion.div>

        {/* Logs toggle button */}
        {logs.length > 0 && (
          <motion.button
            onClick={() => setShowLogs(!showLogs)}
            className={`mt-8 px-6 py-2 rounded-lg text-sm font-medium transition-all duration-300 ${
              isDark ? "bg-cyan-900/30 hover:bg-cyan-900/50 border border-cyan-500/30 text-cyan-300"
                : "bg-slate-100 hover:bg-slate-200 border border-slate-200 text-slate-600"
            }`}
            variants={itemVariants}
          >
            {showLogs ? "إخفاء السجل" : "عرض السجل"}
          </motion.button>
        )}

        {/* Logs display */}
        {showLogs && (
          <div className={`mt-6 w-full max-w-2xl rounded-lg p-4 backdrop-blur-sm ${isDark ? "bg-black/40 border border-cyan-500/20" : "bg-white border border-slate-200"}`}>
            <LogViewer logs={logs} isDark={isDark} maxHeightClass="max-h-48" />
          </div>
        )}
      </motion.div>

      {/* Status Dialog */}
      <StatusDialog
        open={statusDialog}
        onOpenChange={setStatusDialog}
        status={status}
        statusLoading={statusLoading}
        statusResult={statusResult}
        logs={logs}
        isDark={isDark}
      />

      {/* Confetti celebration overlay */}
      {confettiOrigin && celebratingKey && (
        <Confetti
          originX={confettiOrigin.x}
          originY={confettiOrigin.y}
          onComplete={() => setConfettiOrigin(null)}
        />
      )}

      {/* Shimmer keyframes */}
      <style>{`
        @keyframes shimmer {
          0% { transform: translateX(-100%); }
          100% { transform: translateX(100%); }
        }
      `}</style>
    </div>
  );
}
