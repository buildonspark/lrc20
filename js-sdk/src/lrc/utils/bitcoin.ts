import { BitcoinUtxo } from "../types/bitcoin-utxo";

export function filterUniqueUtxo<T extends BitcoinUtxo>(oldUtxos: Array<T>, newUtxos: Array<T>): Array<T> {
  return newUtxos.filter((newUtxo) => {
    return oldUtxos.findIndex((oldUtxo) => oldUtxo.txid == newUtxo.txid && oldUtxo.vout == newUtxo.vout) == -1;
  });
}
