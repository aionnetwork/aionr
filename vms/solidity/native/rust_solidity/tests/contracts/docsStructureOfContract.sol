pragma solidity ^0.4.0;

contract SimpleStorage {
    uint storedData; // State variable
    // ...
}

contract Purchase {
    enum State { Created, Locked, Inactive } // Enum
    
    address public seller;

    modifier onlySeller() { // Modifier
        require(msg.sender == seller);
        _;
    }

    function abort() onlySeller { // Modifier usage
        // ...
    }
}

contract SimpleAuction {
    event HighestBidIncreased(address bidder, uint amount); // Event

    function bid() payable {
        // ...
        HighestBidIncreased(msg.sender, msg.value); // Triggering event
    }
}

contract Ballot {
    struct Voter { // Struct
        uint weight;
        bool voted;
        address delegate;
        uint vote;
    }
}