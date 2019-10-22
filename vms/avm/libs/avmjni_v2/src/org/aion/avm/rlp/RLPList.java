package org.aion.avm.rlp;

import org.aion.avm.utils.DecodeResult;
import org.aion.avm.utils.ByteUtil;
import org.aion.avm.utils.Hex;
import org.aion.avm.utils.Value;
import org.aion.avm.utils.Utils;

import java.util.ArrayList;

/**
 * @author Roman Mandeleil
 * @since 21.04.14
 */
public class RLPList extends ArrayList<RLPElement> implements RLPElement {

    private static final long serialVersionUID = -2855280911054117106L;

    byte[] rlpData;

    public void setRLPData(byte[] rlpData) {
        this.rlpData = rlpData;
    }

    public byte[] getRLPData() {
        return rlpData;
    }

    public static void recursivePrint(RLPElement element) {

        if (element == null) {
            throw new RuntimeException("RLPElement object can't be null");
        }
        if (element instanceof RLPList) {
            RLPList rlpList = (RLPList) element;
            System.out.print("[");
            for (RLPElement singleElement : rlpList) {
                recursivePrint(singleElement);
            }
            System.out.print("]");
        } else {
            String hex = ByteUtil.toHexString(element.getRLPData());
            System.out.print(hex + ", ");
        }
    }
}
