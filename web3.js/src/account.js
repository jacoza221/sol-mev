// @flow
import nacl from 'tweetnacl';
import bs58 from 'bs58';
import type {KeyPair} from 'tweetnacl';

/**
 * @typedef {string} PublicKey
 */
export type PublicKey = string;

export class Account {
  _keypair: KeyPair;

  constructor(secretKey: ?Buffer = null) {
    if (secretKey) {
      this._keypair = nacl.sign.keyPair.fromSecretKey(secretKey);
    } else {
      this._keypair = nacl.sign.keyPair();
    }
  }

  get publicKey(): PublicKey {
    return bs58.encode(this._keypair.publicKey);
  }

  get secretKey(): Buffer {
    return this._keypair.secretKey;
  }
}

