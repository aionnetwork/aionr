pragma solidity ^0.4.0;

contract BlockGasLimit {

    address[] private refundAddresses;
    mapping (address => uint) public refunds;
    uint payIndex;

    function send(uint limit) public {
        uint x = payIndex;
        while(x < refundAddresses.length && msg.gas > limit) {
            if(refunds[refundAddresses[x]] > limit) {
                revert();
            }

            refunds[x] += refunds[refundAddresses[x]];
            x++;
        }
        payIndex = x;

    }
}

