// This contract tests internal transactions (contract creats, 
// calls and transfers value to another contract)
// Aion Foundation

pragma solidity ^0.4.10;

contract NewContract {
    
    event DefaultCalled();
    event PlusOne(uint n);
    uint public result;

    function plusOne(uint n) returns (uint) {
        result = n + 1;
        PlusOne(result);
        return result;
    }

    function() {
        DefaultCalled();
    }
}

contract InternalTransaction {
    
    event TransferValue(uint n);
    event PayValue(uint n);
    event DeployNewContract(address n);
    event Deposit(uint n);
    event PlusTwo(uint n);

    function InternalTransaction() payable {}

    function transferValue(address a) payable {
        a.transfer(msg.value);
        TransferValue(msg.value);
    }

    function payValue(address a, uint n) {
        a.transfer(n);
        PayValue(n);
    }

    function create() returns (address) {
        address new_contract = new NewContract();
        DeployNewContract(new_contract);
        return new_contract;
    }

    function plusTwo(address a, uint n) returns (uint) {
        NewContract new_contract = NewContract(a);
        var result = new_contract.plusOne(n) + 1;
        PlusTwo(result);
        return result;
    }

    function() payable {
        Deposit(msg.value);
    }
}