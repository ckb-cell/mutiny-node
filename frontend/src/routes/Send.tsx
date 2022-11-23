import { useState } from "react";
import { useNavigate } from "react-router";
import Close from "../components/Close";
import PageTitle from "../components/PageTitle";
import ScreenMain from "../components/ScreenMain";
import { inputStyle } from "../styles";
import toast from "react-hot-toast"
import MutinyToaster from "../components/MutinyToaster";
import { detectPaymentType, PaymentType } from "@util/dumb";

function Send() {
  let navigate = useNavigate();

  const [destination, setDestination] = useState("")

  function handleContinue() {
    if (!destination) {
      toast("You didn't paste anything!");
      return
    }

    let paymentType = detectPaymentType(destination)

    if (paymentType === PaymentType.invoice) {
      toast("We don't support invoices yet")
      return
    }

    if (paymentType === PaymentType.unknown) {
      toast("Couldn't parse that one, buddy")
      return
    }

    navigate(`/send/amount?destination=${destination}`);
  }
  return (
    <>
      <header className='p-8 flex justify-between items-center'>
        <PageTitle title="Send" theme="green" />
        <Close />
      </header>
      <ScreenMain>
        <div />
        <input onChange={e => setDestination(e.target.value)} className={`w-full ${inputStyle({ accent: "green" })}`} type="text" placeholder='Paste pubkey or address' />
        <div className='flex justify-start'>
          <button onClick={handleContinue}>Continue</button>
        </div>
      </ScreenMain>
      <MutinyToaster />
    </>
  );
}

export default Send;