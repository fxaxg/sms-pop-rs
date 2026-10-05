import { useState } from "react";
import { openBluetoothSettings } from "../../shared/api";
import { useT } from "../../shared/i18n";
import { Disclosure, Modal } from "../components";

export function ConnectionGuide({ onClose }: { onClose: () => void }) {
  const t = useT();
  const [step, setStep] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const steps = [
    [t.connection.step1, t.connection.step1Ok],
    [t.connection.step2, t.connection.step2Desc],
    [t.ui.shareTitle, t.connection.tipsBody],
    [t.connection.step3, t.connection.step3Desc],
  ];
  return <Modal title={t.connection.addDevice} onClose={onClose}>
    <ol className="wizard-progress">{steps.map((_, index) => <li key={index} className={index <= step ? "active" : ""}>{index + 1}</li>)}</ol>
    <h3>{steps[step][0]}</h3><p className="wizard-description">{steps[step][1]}</p>
    {step === 0 && <button className="btn" onClick={async () => { try { await openBluetoothSettings(); } catch (err) { setError(String(err)); } }}>{t.connection.openBluetooth}</button>}
    {step === 3 && <p className="wizard-note">{t.ui.verifyNote}</p>}
    <Disclosure title={t.connection.tipsLabel}><p>{t.connection.tipsBody}</p><p>{t.ui.guideHelp}</p></Disclosure>
    {error && <p className="bad-text" role="alert">{error}</p>}
    <footer className="modal-actions"><button className="btn" disabled={step === 0} onClick={() => setStep(step - 1)}>{t.ui.back}</button><button className="btn primary" onClick={() => step === 3 ? onClose() : setStep(step + 1)}>{step === 3 ? t.ui.done : t.ui.next}</button></footer>
  </Modal>;
}
