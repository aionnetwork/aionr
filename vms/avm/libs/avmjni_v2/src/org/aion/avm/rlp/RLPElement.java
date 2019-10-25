package org.aion.avm.rlp;

import java.io.Serializable;

/**
 * @author Roman Mandeleil 2014
 * @author modified by aion 2017
 */
public interface RLPElement extends Serializable {

    byte[] getRLPData();
}
