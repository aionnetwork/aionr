pragma solidity ^0.4.0;

contract CrossFunction {

    struct Transfer {
        uint to;
        uint from;
        int amount;
    }

    mapping(Transfer => uint) private transfers;

    function r1() {

    }

    function r2(uint amt, uint acctTo, uint acctFrom) {

        if(amt <= 0) {
            throw;
        } else {
            transfers.push(Transfer({to: acctTo, from: acctFrom, amount: amt}));
            transfers[acctTo].amount += amt;
            transfers[acctFrom].amount -= amt;
        }

        amt = 0;
    }
}


