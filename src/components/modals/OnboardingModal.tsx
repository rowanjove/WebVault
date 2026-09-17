import React, { useState } from 'react';
import {
  X,
  Sparkles,
  ArrowRight,
  ArrowLeft,
  Check,
  Globe,
  Clock,
  GitCompare,
  Activity,
  Layers,
  FileDown,
  ShieldCheck,
  Zap,
  Sliders,
  ChevronRight,
  Play,
  RotateCw,
  Plus,
} from 'lucide-react';
import { useAppStore } from '../../stores/useAppStore';

interface StepData {
  id: number;
  badge: string;
  badgeColor: string;
  title: string;
  subtitle: string;
  description: string;
  icon: React.ElementType;
}

export const OnboardingModal: React.FC = () => {
  const {
    isOnboardingOpen,
    setIsOnboardingOpen,
    setIsAddSiteOpen,
    openTimeMachine,
    setCurrentTab,
  } = useAppStore();

  const [currentStep, setCurrentStep] = useState(0);
  const [dontShowAgain, setDontShowAgain] = useState(true);

  if (!isOnboardingOpen) return null;

  const handleClose = () => {
    if (dontShowAgain) {
      localStorage.setItem('webvault_tour_completed', 'true');
    }
    setIsOnboardingOpen(false);
  };

  const steps: StepData[] = [
    {
      id: 0,
      badge: '本地优先 · 真实会话归档',
      badgeColor: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/20',
      title: '欢迎使用 WebVault 网页时光机',
      subtitle: '不仅仅是下载 HTML，而是捕获完整真实的网络访问会话',
      description:
        'WebVault 运行在本地桌面，通过真实 Chromium 浏览器引擎（CDP 协议）完整拦截网络请求，保存 HTML、CSS、JS、动态 API XHR、字体与音视频分片，写入 WARC / WACZ 国际通用归档格式。',
      icon: Sparkles,
    },
    {
      id: 1,
      badge: '核心链路 1 · 数据采集',
      badgeColor: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
      title: '单页快照与全域爬虫抓取',
      subtitle: '支持即时单页捕获、全站递归探索、自动滚动与登录凭证注入',
      description:
        '在顶部输入 URL 即可开启快速捕获；或在【添加站点】中自定义抓取深度、域名范围、页面数上限与自动滚动加载，轻松收录包含懒加载与无限滚动的现代网页。',
      icon: Globe,
    },
    {
      id: 2,
      badge: '核心链路 2 · 时空回溯',
      badgeColor: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/20',
      title: '一体化网页时光机工作台',
      subtitle: 'Wayback 风格版本选择器，原地穿梭回放，告别跨页横跳',
      description:
        '在【网页时光机】工作台中，左侧快速挑选站点与网页，顶部通过日期时间下拉框或【< 前一版 / 后一版 >】按钮平滑切换历史版本，中间直接在沙盒 iframe 中离线回放。',
      icon: Clock,
    },
    {
      id: 3,
      badge: '核心链路 3 · 变化洞察',
      badgeColor: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
      title: '多维版本差异比对 (Diff Engine)',
      subtitle: '文本、视觉、DOM 结构与资源变动的 4 维智能分析',
      description:
        '在时光机中一键切换【版本比对】模式，无需离开当前页面。系统自动计算 0~100 变动评分，提供行级增删高亮、像素级视觉差分图、DOM 树结构变化与新增/移除资源清单。',
      icon: GitCompare,
    },
    {
      id: 4,
      badge: '核心链路 4 · 全自动化',
      badgeColor: 'bg-purple-500/10 text-purple-400 border-purple-500/20',
      title: '网页定时监控与完全脱机归档',
      subtitle: '一键设为监控自动巡检，支持 SingleFile HTML / PDF / WACZ 导出',
      description:
        '在任意页面卡片或时光机顶部点击【设为监控】，即可开启定时自动巡检并捕获更新；支持将快照一键导出为完全脱机的单 HTML 文件、高清 PDF 或标准化 WACZ 归档包。',
      icon: Activity,
    },
  ];

  const current = steps[currentStep];
  const Icon = current.icon;
  const isLast = currentStep === steps.length - 1;

  const handleNext = () => {
    if (isLast) {
      handleClose();
    } else {
      setCurrentStep((prev) => prev + 1);
    }
  };

  const handlePrev = () => {
    if (currentStep > 0) {
      setCurrentStep((prev) => prev - 1);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-xs p-4 select-none">
      <div className="bg-neutral-900 border border-neutral-800 rounded-2xl max-w-2xl w-full p-6 md:p-7 shadow-2xl space-y-6 relative overflow-hidden">
        {/* Background decorative glow */}
        <div className="absolute -right-20 -top-20 w-64 h-64 bg-indigo-600/10 rounded-full blur-3xl pointer-events-none" />
        <div className="absolute -left-20 -bottom-20 w-64 h-64 bg-emerald-600/10 rounded-full blur-3xl pointer-events-none" />

        {/* Top Header */}
        <div className="flex items-center justify-between pb-3 border-b border-neutral-800/80 relative z-10">
          <div className="flex items-center gap-2">
            <span
              className={`text-xs px-2.5 py-0.5 rounded-full border font-mono font-medium ${current.badgeColor}`}
            >
              {current.badge}
            </span>
            <span className="text-xs text-neutral-400 font-mono">
              步骤 {currentStep + 1} / {steps.length}
            </span>
          </div>

          <button
            onClick={handleClose}
            className="text-neutral-400 hover:text-neutral-200 p-1.5 rounded-lg hover:bg-neutral-800 transition cursor-pointer"
            title="关闭指引"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Step Body */}
        <div className="space-y-4 relative z-10 min-h-[260px]">
          {/* Title & Subtitle */}
          <div className="flex items-start gap-4">
            <div className="p-3 bg-neutral-950 border border-neutral-800 rounded-xl shrink-0 shadow-xs">
              <Icon className="w-6 h-6 text-indigo-400" />
            </div>
            <div className="space-y-1">
              <h2 className="text-lg md:text-xl font-bold text-neutral-100">{current.title}</h2>
              <div className="text-xs md:text-sm text-indigo-400/90 font-medium">
                {current.subtitle}
              </div>
            </div>
          </div>

          {/* Description */}
          <p className="text-xs md:text-sm text-neutral-300 leading-relaxed pt-1">
            {current.description}
          </p>

          {/* Interactive Feature Visual per Step */}
          {currentStep === 0 && (
            <div className="grid grid-cols-3 gap-3 pt-3">
              <div className="p-3 bg-neutral-950/80 border border-neutral-800/80 rounded-xl space-y-1.5 shadow-2xs">
                <div className="flex items-center gap-2 text-xs font-semibold text-neutral-200">
                  <Zap className="w-3.5 h-3.5 text-amber-400" />
                  真实会话录制
                </div>
                <div className="text-[11px] text-neutral-400 leading-relaxed">
                  保存 JS 渲染后的动态 DOM 与异步请求数据，绝不破坏现代 SPA 网页。
                </div>
              </div>

              <div className="p-3 bg-neutral-950/80 border border-neutral-800/80 rounded-xl space-y-1.5 shadow-2xs">
                <div className="flex items-center gap-2 text-xs font-semibold text-neutral-200">
                  <Layers className="w-3.5 h-3.5 text-cyan-400" />
                  WARC / WACZ
                </div>
                <div className="text-[11px] text-neutral-400 leading-relaxed">
                  国际通用网页归档标准，支持打包互通与第三方归档软件直接读取。
                </div>
              </div>

              <div className="p-3 bg-neutral-950/80 border border-neutral-800/80 rounded-xl space-y-1.5 shadow-2xs">
                <div className="flex items-center gap-2 text-xs font-semibold text-neutral-200">
                  <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
                  本地 100% 私有
                </div>
                <div className="text-[11px] text-neutral-400 leading-relaxed">
                  数据库与归档数据完整存放在本地磁盘，完全脱机可用，不依赖云端。
                </div>
              </div>
            </div>
          )}

          {currentStep === 1 && (
            <div className="bg-neutral-950/80 border border-neutral-800/80 rounded-xl p-3.5 space-y-2.5 pt-3">
              <div className="text-xs font-semibold text-neutral-200 flex items-center justify-between">
                <span>抓取工作流一览</span>
                <button
                  onClick={() => {
                    handleClose();
                    setIsAddSiteOpen(true);
                  }}
                  className="text-xs text-indigo-400 hover:text-indigo-300 font-medium flex items-center gap-1 cursor-pointer"
                >
                  <Plus className="w-3.5 h-3.5" />
                  打开添加站点弹窗试一试
                </button>
              </div>

              <div className="grid grid-cols-2 gap-2.5 text-xs text-neutral-300">
                <div className="p-2.5 bg-neutral-900/80 rounded-lg border border-neutral-800 flex items-start gap-2">
                  <span className="w-5 h-5 rounded bg-indigo-600/20 text-indigo-300 flex items-center justify-center font-mono text-[11px] font-bold shrink-0">
                    1
                  </span>
                  <div>
                    <span className="font-semibold block text-neutral-100">输入网址与命名</span>
                    <span className="text-[11px] text-neutral-400">支持直接粘贴剪贴板 URL 或域名</span>
                  </div>
                </div>

                <div className="p-2.5 bg-neutral-900/80 rounded-lg border border-neutral-800 flex items-start gap-2">
                  <span className="w-5 h-5 rounded bg-indigo-600/20 text-indigo-300 flex items-center justify-center font-mono text-[11px] font-bold shrink-0">
                    2
                  </span>
                  <div>
                    <span className="font-semibold block text-neutral-100">选择单页 / 整站模式</span>
                    <span className="text-[11px] text-neutral-400">
                      整站模式可限制爬虫深度与最大页数
                    </span>
                  </div>
                </div>

                <div className="p-2.5 bg-neutral-900/80 rounded-lg border border-neutral-800 flex items-start gap-2">
                  <span className="w-5 h-5 rounded bg-indigo-600/20 text-indigo-300 flex items-center justify-center font-mono text-[11px] font-bold shrink-0">
                    3
                  </span>
                  <div>
                    <span className="font-semibold block text-neutral-100">自动滚动加载</span>
                    <span className="text-[11px] text-neutral-400">
                      模拟人类滚动视口，触发图片与数据懒加载
                    </span>
                  </div>
                </div>

                <div className="p-2.5 bg-neutral-900/80 rounded-lg border border-neutral-800 flex items-start gap-2">
                  <span className="w-5 h-5 rounded bg-indigo-600/20 text-indigo-300 flex items-center justify-center font-mono text-[11px] font-bold shrink-0">
                    4
                  </span>
                  <div>
                    <span className="font-semibold block text-neutral-100">任务队列直达回放</span>
                    <span className="text-[11px] text-neutral-400">
                      任务完成后卡片直接提供【进入时光机】
                    </span>
                  </div>
                </div>
              </div>
            </div>
          )}

          {currentStep === 2 && (
            <div className="bg-neutral-950/80 border border-neutral-800/80 rounded-xl p-3.5 space-y-2.5 pt-3">
              <div className="text-xs font-semibold text-neutral-200 flex items-center justify-between">
                <span>时光机工作台核心功能</span>
                <button
                  onClick={() => {
                    handleClose();
                    openTimeMachine();
                  }}
                  className="text-xs text-indigo-400 hover:text-indigo-300 font-medium flex items-center gap-1 cursor-pointer"
                >
                  <Clock className="w-3.5 h-3.5" />
                  立即前往时光机工作台
                </button>
              </div>

              <div className="space-y-2 text-xs">
                <div className="p-2.5 bg-neutral-900/70 border border-neutral-800 rounded-lg flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="px-2 py-0.5 rounded bg-neutral-800 font-mono text-neutral-200 text-[11px] font-semibold">
                      顶部时间滑块
                    </span>
                    <span className="text-neutral-300 text-xs">
                      下拉快速选择快照时间，点击【&lt; 前一版】与【后一版 &gt;】原地刷新
                    </span>
                  </div>
                  <ChevronRight className="w-4 h-4 text-neutral-500" />
                </div>

                <div className="p-2.5 bg-neutral-900/70 border border-neutral-800 rounded-lg flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="px-2 py-0.5 rounded bg-neutral-800 font-mono text-neutral-200 text-[11px] font-semibold">
                      三合一视图
                    </span>
                    <span className="text-neutral-300 text-xs">
                      自由切换【网页回放】、【版本比对】与【资源清单】模式
                    </span>
                  </div>
                  <ChevronRight className="w-4 h-4 text-neutral-500" />
                </div>

                <div className="p-2.5 bg-neutral-900/70 border border-neutral-800 rounded-lg flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="px-2 py-0.5 rounded bg-neutral-800 font-mono text-neutral-200 text-[11px] font-semibold">
                      一键设为监控
                    </span>
                    <span className="text-neutral-300 text-xs">
                      在时光机顶部点击即可将正在查看的网页加入自动巡检
                    </span>
                  </div>
                  <ChevronRight className="w-4 h-4 text-neutral-500" />
                </div>
              </div>
            </div>
          )}

          {currentStep === 3 && (
            <div className="grid grid-cols-2 gap-2.5 pt-2 text-xs">
              <div className="p-3 bg-neutral-950/80 border border-neutral-800/80 rounded-xl space-y-1">
                <div className="font-semibold text-emerald-400 flex items-center gap-1.5">
                  <span className="w-2 h-2 rounded-full bg-emerald-400" />
                  文本增删对比
                </div>
                <div className="text-[11px] text-neutral-400 leading-relaxed">
                  精确对比两次快照正文内容，高亮标注新增行（绿）与移除行（红），直观呈现信息改动。
                </div>
              </div>

              <div className="p-3 bg-neutral-950/80 border border-neutral-800/80 rounded-xl space-y-1">
                <div className="font-semibold text-amber-400 flex items-center gap-1.5">
                  <span className="w-2 h-2 rounded-full bg-amber-400" />
                  像素级视觉差分
                </div>
                <div className="text-[11px] text-neutral-400 leading-relaxed">
                  将两版页面截图做像素级差分运算，红色高亮布局错位、元素增减与排版改动。
                </div>
              </div>

              <div className="p-3 bg-neutral-950/80 border border-neutral-800/80 rounded-xl space-y-1">
                <div className="font-semibold text-cyan-400 flex items-center gap-1.5">
                  <span className="w-2 h-2 rounded-full bg-cyan-400" />
                  DOM 树结构分析
                </div>
                <div className="text-[11px] text-neutral-400 leading-relaxed">
                  解析 HTML 标签分布与结构相似度，识别节点层级变化与标签增减分布。
                </div>
              </div>

              <div className="p-3 bg-neutral-950/80 border border-neutral-800/80 rounded-xl space-y-1">
                <div className="font-semibold text-purple-400 flex items-center gap-1.5">
                  <span className="w-2 h-2 rounded-full bg-purple-400" />
                  资源变动清单
                </div>
                <div className="text-[11px] text-neutral-400 leading-relaxed">
                  检查外部引入的图片、样式文件、脚本库或媒体资源是否被替换或失效。
                </div>
              </div>
            </div>
          )}

          {currentStep === 4 && (
            <div className="bg-neutral-950/80 border border-neutral-800/80 rounded-xl p-3.5 space-y-3 pt-3">
              <div className="text-xs font-semibold text-neutral-200">
                自动化与长期数据安全
              </div>

              <div className="grid grid-cols-3 gap-2.5 text-xs text-neutral-300">
                <div className="p-2.5 bg-neutral-900/80 rounded-lg border border-neutral-800 space-y-1">
                  <div className="font-semibold text-neutral-100 flex items-center gap-1.5">
                    <Activity className="w-3.5 h-3.5 text-amber-400" />
                    定时巡检与告警
                  </div>
                  <div className="text-[11px] text-neutral-400">
                    设置检查周期（15分钟/每小时/每天），内容变动时自动记录事件。
                  </div>
                </div>

                <div className="p-2.5 bg-neutral-900/80 rounded-lg border border-neutral-800 space-y-1">
                  <div className="font-semibold text-neutral-100 flex items-center gap-1.5">
                    <FileDown className="w-3.5 h-3.5 text-cyan-400" />
                    SingleFile 脱机
                  </div>
                  <div className="text-[11px] text-neutral-400">
                    一键导出单 HTML 离线文档，脱离软件环境直接在任意浏览器打开。
                  </div>
                </div>

                <div className="p-2.5 bg-neutral-900/80 rounded-lg border border-neutral-800 space-y-1">
                  <div className="font-semibold text-neutral-100 flex items-center gap-1.5">
                    <Layers className="w-3.5 h-3.5 text-indigo-400" />
                    WACZ 归档导出
                  </div>
                  <div className="text-[11px] text-neutral-400">
                    整站打包导出为 WACZ 压缩包，方便长期冷存储、迁移备份与机构共享。
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer Controls */}
        <div className="flex items-center justify-between pt-4 border-t border-neutral-800/80 relative z-10">
          {/* Don't show again checkbox */}
          <label className="flex items-center gap-2 text-xs text-neutral-400 cursor-pointer">
            <input
              type="checkbox"
              checked={dontShowAgain}
              onChange={(e) => setDontShowAgain(e.target.checked)}
              className="accent-indigo-600 rounded w-3.5 h-3.5 cursor-pointer"
            />
            <span>不再主动弹出指引</span>
          </label>

          {/* Steps indicator & Action Buttons */}
          <div className="flex items-center gap-4">
            {/* Step dots */}
            <div className="flex items-center gap-1.5">
              {steps.map((_, idx) => (
                <button
                  key={idx}
                  onClick={() => setCurrentStep(idx)}
                  className={`w-2.5 h-2.5 rounded-full transition-all cursor-pointer ${
                    idx === currentStep
                      ? 'bg-indigo-500 w-5'
                      : idx < currentStep
                      ? 'bg-neutral-600 hover:bg-neutral-500'
                      : 'bg-neutral-800 hover:bg-neutral-700'
                  }`}
                  title={`跳至步骤 ${idx + 1}`}
                />
              ))}
            </div>

            {/* Navigation buttons */}
            <div className="flex items-center gap-2">
              {currentStep > 0 && (
                <button
                  onClick={handlePrev}
                  className="px-3.5 py-2 rounded-lg border border-neutral-800 hover:bg-neutral-800 text-neutral-300 text-xs font-medium transition cursor-pointer flex items-center gap-1"
                >
                  <ArrowLeft className="w-3.5 h-3.5" />
                  上一步
                </button>
              )}

              <button
                onClick={handleNext}
                className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition shadow-xs cursor-pointer flex items-center gap-1.5 active:scale-95"
              >
                <span>{isLast ? '完成指引，立即体验' : '下一步'}</span>
                {isLast ? <Check className="w-3.5 h-3.5" /> : <ArrowRight className="w-3.5 h-3.5" />}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
