pragma solidity ^0.4.15;

/*
    Bancor Converter v0.6

    The Bancor version of the token converter, allows conversion between a smart token and other ERC20 tokens and between different ERC20 tokens and themselves.

    ERC20 connector balance can be virtual, meaning that the calculations are based on the virtual balance instead of relying on
    the actual connector balance. This is a security mechanism that prevents the need to keep a very large (and valuable) balance in a single contract.

    The converter is upgradable (just like any SmartTokenController).

    WARNING: It is NOT RECOMMENDED to use the converter with Smart Tokens that have less than 8 decimal digits
             or with very small numbers because of precision loss


    Open issues:
    - Front-running attacks are currently mitigated by the following mechanisms:
        - minimum return argument for each conversion provides a way to define a minimum/maximum price for the transaction
        - gas price limit prevents users from having control over the order of execution
      Other potential solutions might include a commit/reveal based schemes
    - Possibly add getters for the connector fields so that the client won't need to rely on the order in the struct
*/

/*
    Token Holder interface
*/

/*
    Owned contract interface
*/
contract Utils {
    /**
        constructor
    */
    function Utils() {
    }

    // verifies that an amount is greater than zero
    modifier greaterThanZero(uint128 _amount) {
        require(_amount > 0);
        _;
    }

    // validates an address - currently only checks that it isn't null
    modifier validAddress(address _address) {
        require(_address != 0x0);
        _;
    }

    // verifies that the address is different than this contract address
    modifier notThis(address _address) {
        require(_address != address(this));
        _;
    }

    // Overflow protected math functions

    /**
        @dev returns the sum of _x and _y, asserts if the calculation overflows

        @param _x   value 1
        @param _y   value 2

        @return sum
    */
    function safeAdd(uint128 _x, uint128 _y) internal constant returns (uint128) {
        uint128 z = _x + _y;
        assert(z >= _x);
        return z;
    }

    /**
        @dev returns the difference of _x minus _y, asserts if the subtraction results in a negative number

        @param _x   minuend
        @param _y   subtrahend

        @return difference
    */
    function safeSub(uint128 _x, uint128 _y) internal constant returns (uint128) {
        assert(_x >= _y);
        return _x - _y;
    }

    /**
        @dev returns the product of multiplying _x by _y, asserts if the calculation overflows

        @param _x   factor 1
        @param _y   factor 2

        @return product
    */
    function safeMul(uint128 _x, uint128 _y) internal constant returns (uint128) {
        uint128 z = _x * _y;
        assert(_x == 0 || z / _x == _y);
        return z;
    }
}

contract IOwned {
    // this function isn't abstract since the compiler emits automatically generated getter functions as external
    function owner() public constant returns (address) {}

    function transferOwnership(address _newOwner) public;

    function acceptOwnership() public;
}


/*
    Provides support and utilities for contract ownership
*/
contract Owned is IOwned {
    address public owner;
    address public newOwner;

    event OwnerUpdate(address _prevOwner, address _newOwner);

    /**
        @dev constructor
    */
    function Owned() {
        owner = msg.sender;
    }

    // allows execution by the owner only
    modifier ownerOnly {
        assert(msg.sender == owner);
        _;
    }

    /**
        @dev allows transferring the contract ownership
        the new owner still needs to accept the transfer
        can only be called by the contract owner

        @param _newOwner    new contract owner
    */
    function transferOwnership(address _newOwner) public ownerOnly {
        require(_newOwner != owner);
        newOwner = _newOwner;
    }

    /**
        @dev used by a new owner to accept an ownership transfer
    */
    function acceptOwnership() public {
        require(msg.sender == newOwner);
        OwnerUpdate(owner, newOwner);
        owner = newOwner;
        newOwner = 0x0;
    }
}

contract ITokenHolder is IOwned {
    function withdrawTokens(IERC20Token _token, address _to, uint128 _amount) public;
}

contract IERC20Token {
    // these functions aren't abstract since the compiler emits automatically generated getter functions as external
    function name() public constant returns (string) {}

    function symbol() public constant returns (string) {}

    function decimals() public constant returns (uint8) {}

    function totalSupply() public constant returns (uint128) {}

    function balanceOf(address _owner) public constant returns (uint128) {_owner;}

    function allowance(address _owner, address _spender) public constant returns (uint128) {_owner;
        _spender;}

    function transfer(address _to, uint128 _value) public returns (bool success);

    function transferFrom(address _from, address _to, uint128 _value) public returns (bool success);

    function approve(address _spender, uint128 _value) public returns (bool success);
}

/*
    EIP228 Token Converter interface
*/
contract ITokenConverter {
    function convertibleTokenCount() public constant returns (uint16);

    function convertibleToken(uint16 _tokenIndex) public constant returns (address);

    function getReturn(IERC20Token _fromToken, IERC20Token _toToken, uint128 _amount) public constant returns (uint128);

    function convert(IERC20Token _fromToken, IERC20Token _toToken, uint128 _amount, uint128 _minReturn) public returns (uint128);
    // deprecated, backward compatibility
    function change(IERC20Token _fromToken, IERC20Token _toToken, uint128 _amount, uint128 _minReturn) public returns (uint128);
}


/*
    Smart Token interface
*/
contract ISmartToken is IOwned, IERC20Token {
    function disableTransfers(bool _disable) public;

    function issue(address _to, uint128 _amount) public;

    function destroy(address _from, uint128 _amount) public;
}



/*
    Bancor Quick Converter interface
*/
contract IBancorQuickConverter {
    function convert(IERC20Token[] _path, uint128 _amount, uint128 _minReturn) public payable returns (uint128);

    function convertFor(IERC20Token[] _path, uint128 _amount, uint128 _minReturn, address _for) public payable returns (uint128);
}

/*
    Bancor Gas Price Limit interface
*/
contract IBancorGasPriceLimit {
    function gasPrice() public constant returns (uint128) {}
}


/*
    Bancor Formula interface
*/
contract IBancorFormula {
    function calculatePurchaseReturn(uint128 _supply, uint128 _connectorBalance, uint32 _connectorWeight, uint128 _depositAmount) public constant returns (uint128);

    function calculateSaleReturn(uint128 _supply, uint128 _connectorBalance, uint32 _connectorWeight, uint128 _sellAmount) public constant returns (uint128);
}


/*
    Bancor Converter Extensions interface
*/
contract IBancorConverterExtensions {
    function formula() public constant returns (IBancorFormula) {}

    function gasPriceLimit() public constant returns (IBancorGasPriceLimit) {}

    function quickConverter() public constant returns (IBancorQuickConverter) {}
}

contract TokenHolder is ITokenHolder, Owned, Utils {
    /**
        @dev constructor
    */
    function TokenHolder() {
    }

    /**
        @dev withdraws tokens held by the contract and sends them to an account
        can only be called by the owner

        @param _token   ERC20 token contract address
        @param _to      account to receive the new amount
        @param _amount  amount to withdraw
    */
    function withdrawTokens(IERC20Token _token, address _to, uint128 _amount)
    public
    ownerOnly
    validAddress(_token)
    validAddress(_to)
    notThis(_to)
    {
        assert(_token.transfer(_to, _amount));
    }
}

/*
    The smart token controller is an upgradable part of the smart token that allows
    more functionality as well as fixes for bugs/exploits.
    Once it accepts ownership of the token, it becomes the token's sole controller
    that can execute any of its functions.

    To upgrade the controller, ownership must be transferred to a new controller, along with
    any relevant data.

    The smart token must be set on construction and cannot be changed afterwards.
    Wrappers are provided (as opposed to a single 'execute' function) for each of the token's functions, for easier access.

    Note that the controller can transfer token ownership to a new controller that
    doesn't allow executing any function on the token, for a trustless solution.
    Doing that will also remove the owner's ability to upgrade the controller.
*/
contract SmartTokenController is TokenHolder {
    ISmartToken public token;   // smart token

    /**
        @dev constructor
    */
    function SmartTokenController(ISmartToken _token)
    validAddress(_token)
    {
        token = _token;
    }

    // ensures that the controller is the token's owner
    modifier active() {
        assert(token.owner() == address(this));
        _;
    }

    // ensures that the controller is not the token's owner
    modifier inactive() {
        assert(token.owner() != address(this));
        _;
    }

    /**
        @dev allows transferring the token ownership
        the new owner still need to accept the transfer
        can only be called by the contract owner

        @param _newOwner    new token owner
    */
    function transferTokenOwnership(address _newOwner) public ownerOnly {
        token.transferOwnership(_newOwner);
    }

    /**
        @dev used by a new owner to accept a token ownership transfer
        can only be called by the contract owner
    */
    function acceptTokenOwnership() public ownerOnly {
        token.acceptOwnership();
    }

    /**
        @dev disables/enables token transfers
        can only be called by the contract owner

        @param _disable    true to disable transfers, false to enable them
    */
    function disableTokenTransfers(bool _disable) public ownerOnly {
        token.disableTransfers(_disable);
    }

    /**
        @dev withdraws tokens held by the token and sends them to an account
        can only be called by the owner

        @param _token   ERC20 token contract address
        @param _to      account to receive the new amount
        @param _amount  amount to withdraw
    */
    function withdrawFromToken(IERC20Token _token, address _to, uint128 _amount) public ownerOnly {
        ITokenHolder(token).withdrawTokens(_token, _to, _amount);
    }
}

/*
    Provides support and utilities for contract management
*/
contract Managed {
    address public manager;
    address public newManager;

    event ManagerUpdate(address _prevManager, address _newManager);

    /**
        @dev constructor
    */
    function Managed() {
        manager = msg.sender;
    }

    // allows execution by the manager only
    modifier managerOnly {
        assert(msg.sender == manager);
        _;
    }


    /**
        @dev allows transferring the contract management
        the new manager still needs to accept the transfer
        can only be called by the contract manager

        @param _newManager    new contract manager
    */
    function transferManagement(address _newManager) public managerOnly {
        require(_newManager != manager);
        newManager = _newManager;
    }

    /**
        @dev used by a new manager to accept a management transfer
    */
    function acceptManagement() public {
        require(msg.sender == newManager);
        ManagerUpdate(manager, newManager);
        manager = newManager;
        newManager = 0x0;
    }
}

contract BancorConverter is ITokenConverter, SmartTokenController, Managed {
    uint32 private constant MAX_WEIGHT = 1000000;
    uint32 private constant MAX_CONVERSION_FEE = 1000000;

    struct Connector {
        uint128 virtualBalance;         // connector virtual balance
        uint32 weight;                  // connector weight, represented in ppm, 1-1000000
        bool isVirtualBalanceEnabled;   // true if virtual balance is enabled, false if not
        bool isPurchaseEnabled;         // is purchase of the smart token enabled with the connector, can be set by the owner
        bool isSet;                     // used to tell if the mapping element is defined
    }

    string public version = '0.6';
    string public converterType = 'bancor';

    IBancorConverterExtensions public extensions;       // bancor converter extensions contract
    IERC20Token[] public connectorTokens;               // ERC20 standard token addresses
    IERC20Token[] public quickBuyPath;                  // conversion path that's used in order to buy the token with ETH
    mapping(address => Connector) public connectors;   // connector token addresses -> connector data
    uint32 private totalConnectorWeight = 0;            // used to efficiently prevent increasing the total connector weight above 100%
    uint32 public maxConversionFee = 0;                 // maximum conversion fee for the lifetime of the contract, represented in ppm, 0...1000000 (0 = no fee, 100 = 0.01%, 1000000 = 100%)
    uint32 public conversionFee = 0;                    // current conversion fee, represented in ppm, 0...maxConversionFee
    bool public conversionsEnabled = true;              // true if token conversions is enabled, false if not

    // triggered when a conversion between two tokens occurs (TokenConverter event)
    event Conversion(address indexed _fromToken, address indexed _toToken, address indexed _trader, uint128 _amount, uint128 _return,
        uint128 _currentPriceN, uint128 _currentPriceD);
    // triggered when the conversion fee is updated
    event ConversionFeeUpdate(uint32 _prevFee, uint32 _newFee);

    /**
        @dev constructor

        @param  _token              smart token governed by the converter
        @param  _extensions         address of a bancor converter extensions contract
        @param  _maxConversionFee   maximum conversion fee, represented in ppm
        @param  _connectorToken     optional, initial connector, allows defining the first connector at deployment time
        @param  _connectorWeight    optional, weight for the initial connector
    */
    function BancorConverter(ISmartToken _token, IBancorConverterExtensions _extensions, uint32 _maxConversionFee, IERC20Token _connectorToken, uint32 _connectorWeight)
    SmartTokenController(_token)
    validAddress(_extensions)
    validMaxConversionFee(_maxConversionFee)
    {
        extensions = _extensions;
        maxConversionFee = _maxConversionFee;

        if (address(_connectorToken) != 0x0)
            addConnector(_connectorToken, _connectorWeight, false);
    }

    // validates a connector token address - verifies that the address belongs to one of the connector tokens
    modifier validConnector(IERC20Token _address) {
        require(connectors[_address].isSet);
        _;
    }

    // validates a token address - verifies that the address belongs to one of the convertible tokens
    modifier validToken(IERC20Token _address) {
        require(_address == token || connectors[_address].isSet);
        _;
    }

    // verifies that the gas price is lower than the universal limit
    modifier validGasPrice() {
        assert(tx.gasprice <= extensions.gasPriceLimit().gasPrice());
        _;
    }

    // validates maximum conversion fee
    modifier validMaxConversionFee(uint32 _conversionFee) {
        require(_conversionFee >= 0 && _conversionFee <= MAX_CONVERSION_FEE);
        _;
    }

    // validates conversion fee
    modifier validConversionFee(uint32 _conversionFee) {
        require(_conversionFee >= 0 && _conversionFee <= maxConversionFee);
        _;
    }

    // validates connector weight range
    modifier validConnectorWeight(uint32 _weight) {
        require(_weight > 0 && _weight <= MAX_WEIGHT);
        _;
    }

    // validates a conversion path - verifies that the number of elements is odd and that maximum number of 'hops' is 10
    modifier validConversionPath(IERC20Token[] _path) {
        require(_path.length > 2 && _path.length <= (1 + 2 * 10) && _path.length % 2 == 1);
        _;
    }

    // allows execution only when conversions aren't disabled
    modifier conversionsAllowed {
        assert(conversionsEnabled);
        _;
    }

    /**
        @dev returns the number of connector tokens defined

        @return number of connector tokens
    */
    function connectorTokenCount() public constant returns (uint16) {
        return uint16(connectorTokens.length);
    }

    /**
        @dev returns the number of convertible tokens supported by the contract
        note that the number of convertible tokens is the number of connector token, plus 1 (that represents the smart token)

        @return number of convertible tokens
    */
    function convertibleTokenCount() public constant returns (uint16) {
        return connectorTokenCount() + 1;
    }

    /**
        @dev given a convertible token index, returns its contract address

        @param _tokenIndex  convertible token index

        @return convertible token address
    */
    function convertibleToken(uint16 _tokenIndex) public constant returns (address) {
        if (_tokenIndex == 0)
            return token;
        return connectorTokens[_tokenIndex - 1];
    }

    /*
        @dev allows the owner to update the extensions contract address

        @param _extensions    address of a bancor converter extensions contract
    */
    function setExtensions(IBancorConverterExtensions _extensions)
    public
    ownerOnly
    validAddress(_extensions)
    notThis(_extensions)
    {
        extensions = _extensions;
    }

    /*
        @dev allows the manager to update the quick buy path

        @param _path    new quick buy path, see conversion path format in the BancorQuickConverter contract
    */
    function setQuickBuyPath(IERC20Token[] _path)
    public
    ownerOnly
    validConversionPath(_path)
    {
        quickBuyPath = _path;
    }

    /*
        @dev allows the manager to clear the quick buy path
    */
    function clearQuickBuyPath() public ownerOnly {
        quickBuyPath.length = 0;
    }

    /**
        @dev returns the length of the quick buy path array

        @return quick buy path length
    */
    function getQuickBuyPathLength() public constant returns (uint128) {
        return uint128(quickBuyPath.length);
    }

    /**
        @dev disables the entire conversion functionality
        this is a safety mechanism in case of a emergency
        can only be called by the manager

        @param _disable true to disable conversions, false to re-enable them
    */
    function disableConversions(bool _disable) public managerOnly {
        conversionsEnabled = !_disable;
    }

    /**
        @dev updates the current conversion fee
        can only be called by the manager

        @param _conversionFee new conversion fee, represented in ppm
    */
    function setConversionFee(uint32 _conversionFee)
    public
    managerOnly
    validConversionFee(_conversionFee)
    {
        ConversionFeeUpdate(conversionFee, _conversionFee);
        conversionFee = _conversionFee;
    }

    /*
        @dev returns the conversion fee amount for a given return amount

        @return conversion fee amount
    */
    function getConversionFeeAmount(uint128 _amount) public constant returns (uint128) {
        return safeMul(_amount, conversionFee) / MAX_CONVERSION_FEE;
    }

    /**
        @dev defines a new connector for the token
        can only be called by the owner while the converter is inactive

        @param _token                  address of the connector token
        @param _weight                 constant connector weight, represented in ppm, 1-1000000
        @param _enableVirtualBalance   true to enable virtual balance for the connector, false to disable it
    */
    function addConnector(IERC20Token _token, uint32 _weight, bool _enableVirtualBalance)
    public
    ownerOnly
    inactive
    validAddress(_token)
    notThis(_token)
    validConnectorWeight(_weight)
    {
        require(_token != token && !connectors[_token].isSet && totalConnectorWeight + _weight <= MAX_WEIGHT);
        // validate input

        connectors[_token].virtualBalance = 0;
        connectors[_token].weight = _weight;
        connectors[_token].isVirtualBalanceEnabled = _enableVirtualBalance;
        connectors[_token].isPurchaseEnabled = true;
        connectors[_token].isSet = true;
        connectorTokens.push(_token);
        totalConnectorWeight += _weight;
    }

    /**
        @dev updates one of the token connectors
        can only be called by the owner

        @param _connectorToken         address of the connector token
        @param _weight                 constant connector weight, represented in ppm, 1-1000000
        @param _enableVirtualBalance   true to enable virtual balance for the connector, false to disable it
        @param _virtualBalance         new connector's virtual balance
    */
    function updateConnector(IERC20Token _connectorToken, uint32 _weight, bool _enableVirtualBalance, uint128 _virtualBalance)
    public
    ownerOnly
    validConnector(_connectorToken)
    validConnectorWeight(_weight)
    {
        Connector storage connector = connectors[_connectorToken];
        require(totalConnectorWeight - connector.weight + _weight <= MAX_WEIGHT);
        // validate input

        totalConnectorWeight = totalConnectorWeight - connector.weight + _weight;
        connector.weight = _weight;
        connector.isVirtualBalanceEnabled = _enableVirtualBalance;
        connector.virtualBalance = _virtualBalance;
    }

    /**
        @dev disables purchasing with the given connector token in case the connector token got compromised
        can only be called by the owner
        note that selling is still enabled regardless of this flag and it cannot be disabled by the owner

        @param _connectorToken  connector token contract address
        @param _disable         true to disable the token, false to re-enable it
    */
    function disableConnectorPurchases(IERC20Token _connectorToken, bool _disable)
    public
    ownerOnly
    validConnector(_connectorToken)
    {
        connectors[_connectorToken].isPurchaseEnabled = !_disable;
    }

    /**
        @dev returns the connector's virtual balance if one is defined, otherwise returns the actual balance

        @param _connectorToken  connector token contract address

        @return connector balance
    */
    function getConnectorBalance(IERC20Token _connectorToken)
    public
    constant
    validConnector(_connectorToken)
    returns (uint128)
    {
        Connector storage connector = connectors[_connectorToken];
        return connector.isVirtualBalanceEnabled ? connector.virtualBalance : _connectorToken.balanceOf(this);
    }

    /**
        @dev returns the expected return for converting a specific amount of _fromToken to _toToken

        @param _fromToken  ERC20 token to convert from
        @param _toToken    ERC20 token to convert to
        @param _amount     amount to convert, in fromToken

        @return expected conversion return amount
    */
    function getReturn(IERC20Token _fromToken, IERC20Token _toToken, uint128 _amount) public constant returns (uint128) {
        require(_fromToken != _toToken);
        // validate input

        // conversion between the token and one of its connectors
        if (_toToken == token)
            return getPurchaseReturn(_fromToken, _amount);
        else if (_fromToken == token)
            return getSaleReturn(_toToken, _amount);

        // conversion between 2 connectors
        uint128 purchaseReturnAmount = getPurchaseReturn(_fromToken, _amount);
        return getSaleReturn(_toToken, purchaseReturnAmount, safeAdd(token.totalSupply(), purchaseReturnAmount));
    }

    /**
        @dev returns the expected return for buying the token for a connector token

        @param _connectorToken  connector token contract address
        @param _depositAmount   amount to deposit (in the connector token)

        @return expected purchase return amount
    */
    function getPurchaseReturn(IERC20Token _connectorToken, uint128 _depositAmount)
    public
    constant
    active
    validConnector(_connectorToken)
    returns (uint128)
    {
        Connector storage connector = connectors[_connectorToken];
        require(connector.isPurchaseEnabled);
        // validate input

        uint128 tokenSupply = token.totalSupply();
        uint128 connectorBalance = getConnectorBalance(_connectorToken);
        uint128 amount = extensions.formula().calculatePurchaseReturn(tokenSupply, connectorBalance, connector.weight, _depositAmount);

        // deduct the fee from the return amount
        uint128 feeAmount = getConversionFeeAmount(amount);
        return safeSub(amount, feeAmount);
    }

    /**
        @dev returns the expected return for selling the token for one of its connector tokens

        @param _connectorToken  connector token contract address
        @param _sellAmount      amount to sell (in the smart token)

        @return expected sale return amount
    */
    function getSaleReturn(IERC20Token _connectorToken, uint128 _sellAmount) public constant returns (uint128) {
        return getSaleReturn(_connectorToken, _sellAmount, token.totalSupply());
    }

    /**
        @dev converts a specific amount of _fromToken to _toToken

        @param _fromToken  ERC20 token to convert from
        @param _toToken    ERC20 token to convert to
        @param _amount     amount to convert, in fromToken
        @param _minReturn  if the conversion results in an amount smaller than the minimum return - it is cancelled, must be nonzero

        @return conversion return amount
    */
    function convert(IERC20Token _fromToken, IERC20Token _toToken, uint128 _amount, uint128 _minReturn) public returns (uint128) {
        require(_fromToken != _toToken);
        // validate input

        // conversion between the token and one of its connectors
        if (_toToken == token)
            return buy(_fromToken, _amount, _minReturn);
        else if (_fromToken == token)
            return sell(_toToken, _amount, _minReturn);

        // conversion between 2 connectors
        uint128 purchaseAmount = buy(_fromToken, _amount, 1);
        return sell(_toToken, purchaseAmount, _minReturn);
    }

    /**
        @dev buys the token by depositing one of its connector tokens

        @param _connectorToken  connector token contract address
        @param _depositAmount   amount to deposit (in the connector token)
        @param _minReturn       if the conversion results in an amount smaller than the minimum return - it is cancelled, must be nonzero

        @return buy return amount
    */
    function buy(IERC20Token _connectorToken, uint128 _depositAmount, uint128 _minReturn)
    public
    conversionsAllowed
    validGasPrice
    greaterThanZero(_minReturn)
    returns (uint128)
    {
        uint128 amount = getPurchaseReturn(_connectorToken, _depositAmount);
        assert(amount != 0 && amount >= _minReturn);
        // ensure the trade gives something in return and meets the minimum requested amount

        // update virtual balance if relevant
        Connector storage connector = connectors[_connectorToken];
        if (connector.isVirtualBalanceEnabled)
            connector.virtualBalance = safeAdd(connector.virtualBalance, _depositAmount);

        // transfer _depositAmount funds from the caller in the connector token
        //assert(_connectorToken.transferFrom(msg.sender, this, _depositAmount));
        // issue new funds to the caller in the smart token
        token.issue(msg.sender, amount);

        dispatchConversionEvent(_connectorToken, _depositAmount, amount, true);
        return amount;
    }
    function checkTransfer(IERC20Token _connectorToken, uint128 _depositAmount, uint128 _minReturn) returns (bool){
        assert(_connectorToken.transferFrom(msg.sender, this, _depositAmount));
        //token.issue(msg.sender, _depositAmount);
        return true;
    }

    /**
        @dev sells the token by withdrawing from one of its connector tokens

        @param _connectorToken  connector token contract address
        @param _sellAmount      amount to sell (in the smart token)
        @param _minReturn       if the conversion results in an amount smaller the minimum return - it is cancelled, must be nonzero

        @return sell return amount
    */
    function sell(IERC20Token _connectorToken, uint128 _sellAmount, uint128 _minReturn)
    public
    conversionsAllowed
    validGasPrice
    greaterThanZero(_minReturn)
    returns (uint128)
    {
        require(_sellAmount <= token.balanceOf(msg.sender));
        // validate input

        uint128 amount = getSaleReturn(_connectorToken, _sellAmount);
        assert(amount != 0 && amount >= _minReturn);
        // ensure the trade gives something in return and meets the minimum requested amount

        uint128 tokenSupply = token.totalSupply();
        uint128 connectorBalance = getConnectorBalance(_connectorToken);
        // ensure that the trade will only deplete the connector if the total supply is depleted as well
        assert(amount < connectorBalance || (amount == connectorBalance && _sellAmount == tokenSupply));

        // update virtual balance if relevant
        Connector storage connector = connectors[_connectorToken];
        if (connector.isVirtualBalanceEnabled)
            connector.virtualBalance = safeSub(connector.virtualBalance, amount);

        // destroy _sellAmount from the caller's balance in the smart token
        token.destroy(msg.sender, _sellAmount);
        // transfer funds to the caller in the connector token
        // the transfer might fail if the actual connector balance is smaller than the virtual balance
        //assert(_connectorToken.transfer(msg.sender, amount));

        dispatchConversionEvent(_connectorToken, _sellAmount, amount, false);
        return amount;
    }

    /**
        @dev converts the token to any other token in the bancor network by following a predefined conversion path
        note that when converting from an ERC20 token (as opposed to a smart token), allowance must be set beforehand

        @param _path        conversion path, see conversion path format in the BancorQuickConverter contract
        @param _amount      amount to convert from (in the initial source token)
        @param _minReturn   if the conversion results in an amount smaller than the minimum return - it is cancelled, must be nonzero

        @return tokens issued in return
    */
    function quickConvert(IERC20Token[] _path, uint128 _amount, uint128 _minReturn)
    public
    payable
    validConversionPath(_path)
    returns (uint128)
    {
        IERC20Token fromToken = _path[0];
        IBancorQuickConverter quickConverter = extensions.quickConverter();

        // we need to transfer the source tokens from the caller to the quick converter,
        // so it can execute the conversion on behalf of the caller
        if (uint128(msg.value) == 0) {
            // not ETH, send the source tokens to the quick converter
            // if the token is the smart token, no allowance is required - destroy the tokens from the caller and issue them to the quick converter
            if (fromToken == token) {
                token.destroy(msg.sender, _amount);
                // destroy _amount tokens from the caller's balance in the smart token
                token.issue(quickConverter, _amount);
                // issue _amount new tokens to the quick converter
            }
            else {
                // otherwise, we assume we already have allowance, transfer the tokens directly to the quick converter
                assert(fromToken.transferFrom(msg.sender, quickConverter, _amount));
            }
        }

        // execute the conversion and pass on the ETH with the call
        return quickConverter.convertFor.value(uint128(msg.value))(_path, _amount, _minReturn, msg.sender);
    }

    // deprecated, backward compatibility
    function change(IERC20Token _fromToken, IERC20Token _toToken, uint128 _amount, uint128 _minReturn) public returns (uint128) {
        return convert(_fromToken, _toToken, _amount, _minReturn);
    }

    /**
        @dev utility, returns the expected return for selling the token for one of its connector tokens, given a total supply override

        @param _connectorToken  connector token contract address
        @param _sellAmount      amount to sell (in the smart token)
        @param _totalSupply     total token supply, overrides the actual token total supply when calculating the return

        @return sale return amount
    */
    function getSaleReturn(IERC20Token _connectorToken, uint128 _sellAmount, uint128 _totalSupply)
    private
    constant
    active
    validConnector(_connectorToken)
    greaterThanZero(_totalSupply)
    returns (uint128)
    {
        Connector storage connector = connectors[_connectorToken];
        uint128 connectorBalance = getConnectorBalance(_connectorToken);
        uint128 amount = extensions.formula().calculateSaleReturn(_totalSupply, connectorBalance, connector.weight, _sellAmount);

        // deduct the fee from the return amount
        uint128 feeAmount = getConversionFeeAmount(amount);
        return safeSub(amount, feeAmount);
    }

    /**
        @dev helper, dispatches the Conversion event
        The function also takes the tokens' decimals into account when calculating the current price

        @param _connectorToken  connector token contract address
        @param _amount          amount purchased/sold (in the source token)
        @param _returnAmount    amount returned (in the target token)
        @param isPurchase       true if it's a purchase, false if it's a sale
    */
    function dispatchConversionEvent(IERC20Token _connectorToken, uint128 _amount, uint128 _returnAmount, bool isPurchase) private {
        Connector storage connector = connectors[_connectorToken];

        // calculate the new price using the simple price formula
        // price = connector balance / (supply * weight)
        // weight is represented in ppm, so multiplying by 1000000
        uint128 connectorAmount = safeMul(getConnectorBalance(_connectorToken), MAX_WEIGHT);
        uint128 tokenAmount = safeMul(token.totalSupply(), connector.weight);

        // normalize values
        uint8 tokenDecimals = token.decimals();
        uint128 decimals = 10;
        uint8 connectorTokenDecimals = _connectorToken.decimals();
        if (tokenDecimals != connectorTokenDecimals) {
            if (tokenDecimals > connectorTokenDecimals)
                connectorAmount = safeMul(connectorAmount, uint128(decimals ** uint128(tokenDecimals - connectorTokenDecimals)));
            else
                tokenAmount = safeMul(tokenAmount, uint128(decimals ** uint128(connectorTokenDecimals - tokenDecimals)));
        }

        // if (isPurchase)
        //     Conversion(_connectorToken, token, msg.sender, _amount, _returnAmount, connectorAmount, tokenAmount);
        // else
        //     Conversion(token, _connectorToken, msg.sender, _amount, _returnAmount, tokenAmount, connectorAmount);
    }

    /**
        @dev fallback, buys the smart token with ETH
        note that the purchase will use the price at the time of the purchase
    */
    function() payable {
        quickConvert(quickBuyPath, uint128(msg.value), 1);
    }
}


/**
    @dev the BancorConverterExtensions contract is an owned contract that serves as a single point of access
    to the BancorFormula, BancorGasPriceLimit and BancorQuickConverter contracts from all BancorConverter contract instances.
    it allows upgrading these contracts without the need to update each and every
    BancorConverter contract instance individually.
*/
contract BancorConverterExtensions is IBancorConverterExtensions, TokenHolder {
    IBancorFormula public formula;  // bancor calculation formula contract
    IBancorGasPriceLimit public gasPriceLimit; // bancor universal gas price limit contract
    IBancorQuickConverter public quickConverter; // bancor quick converter contract

    /**
        @dev constructor

        @param _formula         address of a bancor formula contract
        @param _gasPriceLimit   address of a bancor gas price limit contract
        @param _quickConverter  address of a bancor quick converter contract
    */
    function BancorConverterExtensions(IBancorFormula _formula, IBancorGasPriceLimit _gasPriceLimit, IBancorQuickConverter _quickConverter)
    validAddress(_formula)
    validAddress(_gasPriceLimit)
    validAddress(_quickConverter)
    {
        formula = _formula;
        gasPriceLimit = _gasPriceLimit;
        quickConverter = _quickConverter;
    }

    /*
        @dev allows the owner to update the formula contract address

        @param _formula    address of a bancor formula contract
    */
    function setFormula(IBancorFormula _formula)
    public
    ownerOnly
    validAddress(_formula)
    notThis(_formula)
    {
        formula = _formula;
    }

    /*
        @dev allows the owner to update the gas price limit contract address

        @param _gasPriceLimit   address of a bancor gas price limit contract
    */
    function setGasPriceLimit(IBancorGasPriceLimit _gasPriceLimit)
    public
    ownerOnly
    validAddress(_gasPriceLimit)
    notThis(_gasPriceLimit)
    {
        gasPriceLimit = _gasPriceLimit;
    }

    /*
        @dev allows the owner to update the quick converter contract address

        @param _quickConverter  address of a bancor quick converter contract
    */
    function setQuickConverter(IBancorQuickConverter _quickConverter)
    public
    ownerOnly
    validAddress(_quickConverter)
    notThis(_quickConverter)
    {
        quickConverter = _quickConverter;
    }
}

contract BancorFormula is IBancorFormula, Utils {
    string public version = '0.3';

    uint128 private constant ONE = 1;
    uint32 private constant MAX_WEIGHT = 1000000;
    uint8 private constant MIN_PRECISION = 1;
    uint8 private constant MAX_PRECISION = 63;

    /**
        The values below depend on MAX_PRECISION. If you choose to change it:
        Apply the same change in file 'PrintIntScalingFactors.py', run it and paste the results below.
    */
    uint128 private constant FIXED_1 = 0x08000000000000000;
    uint128 private constant FIXED_2 = 0x10000000000000000;
    uint128 private constant MAX_NUM = 0x1ffffffffffffffff;

    /**
        The values below depend on MAX_PRECISION. If you choose to change it:
        Apply the same change in file 'PrintLn2ScalingFactors.py', run it and paste the results below.
    */
    uint128 private constant LN2_MANTISSA = 0x58b90bfbe8e7bcd;
    uint8   private constant LN2_EXPONENT = 59;

    /**
        The values below depend on MIN_PRECISION and MAX_PRECISION. If you choose to change either one of them:
        Apply the same change in file 'PrintFunctionBancorFormula.py', run it and paste the results below.
    */
    uint128[64] private maxExpArray;

    function BancorFormula() {
        //  maxExpArray[ 0] = 0x1ffffffffffffffff;
        maxExpArray[1] = 0x1bfffffffffffffff;
        maxExpArray[2] = 0x15fffffffffffffff;
        maxExpArray[3] = 0x10fffffffffffffff;
        maxExpArray[4] = 0x0d7ffffffffffffff;
        maxExpArray[5] = 0x09fffffffffffffff;
        maxExpArray[6] = 0x079ffffffffffffff;
        maxExpArray[7] = 0x059ffffffffffffff;
        maxExpArray[8] = 0x040ffffffffffffff;
        maxExpArray[9] = 0x02f7fffffffffffff;
        maxExpArray[10] = 0x021ffffffffffffff;
        maxExpArray[11] = 0x0185fffffffffffff;
        maxExpArray[12] = 0x01157ffffffffffff;
        maxExpArray[13] = 0x00c6bffffffffffff;
        maxExpArray[14] = 0x008c7ffffffffffff;
        maxExpArray[15] = 0x0063effffffffffff;
        maxExpArray[16] = 0x0046a7fffffffffff;
        maxExpArray[17] = 0x003247fffffffffff;
        maxExpArray[18] = 0x00238dfffffffffff;
        maxExpArray[19] = 0x001923fffffffffff;
        maxExpArray[20] = 0x0011c6fffffffffff;
        maxExpArray[21] = 0x000c91fffffffffff;
        maxExpArray[22] = 0x0008e37ffffffffff;
        maxExpArray[23] = 0x000648effffffffff;
        maxExpArray[24] = 0x000471b7fffffffff;
        maxExpArray[25] = 0x00032477fffffffff;
        maxExpArray[26] = 0x000238d9fffffffff;
        maxExpArray[27] = 0x0001923bfffffffff;
        maxExpArray[28] = 0x00011c6c7ffffffff;
        maxExpArray[29] = 0x0000c91dfffffffff;
        maxExpArray[30] = 0x00008e361ffffffff;
        maxExpArray[31] = 0x0000648efffffffff;
        maxExpArray[32] = 0x0000471b07fffffff;
        maxExpArray[33] = 0x000032477bfffffff;
        maxExpArray[34] = 0x0000238d83fffffff;
        maxExpArray[35] = 0x00001923bdfffffff;
        maxExpArray[36] = 0x000011c6c17ffffff;
        maxExpArray[37] = 0x00000c91debffffff;
        maxExpArray[38] = 0x000008e360bffffff;
        maxExpArray[39] = 0x00000648ef5ffffff;
        maxExpArray[40] = 0x00000471b05ffffff;
        maxExpArray[41] = 0x0000032477affffff;
        maxExpArray[42] = 0x00000238d82dfffff;
        maxExpArray[43] = 0x000001923bd7fffff;
        maxExpArray[44] = 0x0000011c6c16fffff;
        maxExpArray[45] = 0x000000c91debfffff;
        maxExpArray[46] = 0x0000008e360b5ffff;
        maxExpArray[47] = 0x000000648ef5fffff;
        maxExpArray[48] = 0x000000471b05affff;
        maxExpArray[49] = 0x00000032477afffff;
        maxExpArray[50] = 0x000000238d82d7fff;
        maxExpArray[51] = 0x0000001923bd7efff;
        maxExpArray[52] = 0x00000011c6c16b7ff;
        maxExpArray[53] = 0x0000000c91debf3ff;
        maxExpArray[54] = 0x00000008e360b59ff;
        maxExpArray[55] = 0x0000000648ef5f8ff;
        maxExpArray[56] = 0x0000000471b05acff;
        maxExpArray[57] = 0x000000032477afc7f;
        maxExpArray[58] = 0x0000000238d82d65f;
        maxExpArray[59] = 0x00000001923bd7e2f;
        maxExpArray[60] = 0x000000011c6c16b2f;
        maxExpArray[61] = 0x00000000c91debf13;
        maxExpArray[62] = 0x000000008e360b597;
        maxExpArray[63] = 0x00000000648ef5f89;
    }

    /**
        @dev given a token supply, connector balance, weight and a deposit amount (in the connector token),
        calculates the return for a given conversion (in the main token)

        Formula:
        Return = _supply * ((1 + _depositAmount / _connectorBalance) ^ (_connectorWeight / 1000000) - 1)

        @param _supply              token total supply
        @param _connectorBalance    total connector balance
        @param _connectorWeight     connector weight, represented in ppm, 1-1000000
        @param _depositAmount       deposit amount, in connector token

        @return purchase return amount
    */
    function calculatePurchaseReturn(uint128 _supply, uint128 _connectorBalance, uint32 _connectorWeight, uint128 _depositAmount) public constant returns (uint128) {
        // validate input
        require(_supply > 0 && _connectorBalance > 0 && _connectorWeight > 0 && _connectorWeight <= MAX_WEIGHT);

        // special case for 0 deposit amount
        if (_depositAmount == 0)
            return 0;

        // special case if the weight = 100%
        if (_connectorWeight == MAX_WEIGHT)
            return safeMul(_supply, _depositAmount) / _connectorBalance;

        uint128 result;
        uint8 precision;
        uint128 baseN = safeAdd(_depositAmount, _connectorBalance);
        (result, precision) = power(baseN, _connectorBalance, _connectorWeight, MAX_WEIGHT);
        uint128 temp = safeMul(_supply, result) >> precision;
        return temp - _supply;
    }

    /**
        @dev given a token supply, connector balance, weight and a sell amount (in the main token),
        calculates the return for a given conversion (in the connector token)

        Formula:
        Return = _connectorBalance * (1 - (1 - _sellAmount / _supply) ^ (1 / (_connectorWeight / 1000000)))

        @param _supply              token total supply
        @param _connectorBalance    total connector
        @param _connectorWeight     constant connector Weight, represented in ppm, 1-1000000
        @param _sellAmount          sell amount, in the token itself

        @return sale return amount
    */
    function calculateSaleReturn(uint128 _supply, uint128 _connectorBalance, uint32 _connectorWeight, uint128 _sellAmount) public constant returns (uint128) {
        // validate input
        require(_supply > 0 && _connectorBalance > 0 && _connectorWeight > 0 && _connectorWeight <= MAX_WEIGHT && _sellAmount <= _supply);

        // special case for 0 sell amount
        if (_sellAmount == 0)
            return 0;

        // special case for selling the entire supply
        if (_sellAmount == _supply)
            return _connectorBalance;

        // special case if the weight = 100%
        if (_connectorWeight == MAX_WEIGHT)
            return safeMul(_connectorBalance, _sellAmount) / _supply;

        uint128 result;
        uint8 precision;
        uint128 baseD = _supply - _sellAmount;
        (result, precision) = power(_supply, baseD, MAX_WEIGHT, _connectorWeight);
        uint128 temp1 = safeMul(_connectorBalance, result);
        uint128 temp2 = _connectorBalance << precision;
        return (temp1 - temp2) / result;
    }

    /**
        General Description:
            Determine a value of precision.
            Calculate an integer approximation of (_baseN / _baseD) ^ (_expN / _expD) * 2 ^ precision.
            Return the result along with the precision used.

        Detailed Description:
            Instead of calculating "base ^ exp", we calculate "e ^ (ln(base) * exp)".
            The value of "ln(base)" is represented with an integer slightly smaller than "ln(base) * 2 ^ precision".
            The larger "precision" is, the more accurately this value represents the real value.
            However, the larger "precision" is, the more bits are required in order to store this value.
            And the exponentiation function, which takes "x" and calculates "e ^ x", is limited to a maximum exponent (maximum value of "x").
            This maximum exponent depends on the "precision" used, and it is given by "maxExpArray[precision] >> (MAX_PRECISION - precision)".
            Hence we need to determine the highest precision which can be used for the given input, before calling the exponentiation function.
            This allows us to compute "base ^ exp" with maximum accuracy and without exceeding 128 bits in any of the intermediate computations.
    */
    function power(uint128 _baseN, uint128 _baseD, uint32 _expN, uint32 _expD) internal constant returns (uint128, uint8) {
        uint128 lnBaseTimesExp = ln(_baseN, _baseD) * _expN / _expD;
        uint8 precision = findPositionInMaxExpArray(lnBaseTimesExp);
        return (fixedExp(lnBaseTimesExp >> (MAX_PRECISION - precision), precision), precision);
    }

    /**
        Return floor(ln(numerator / denominator) * 2 ^ MAX_PRECISION), where:
        - The numerator   is a value between 1 and 2 ^ (128 - MAX_PRECISION) - 1
        - The denominator is a value between 1 and 2 ^ (128 - MAX_PRECISION) - 1
        - The output      is a value between 0 and floor(ln(2 ^ (128 - MAX_PRECISION) - 1) * 2 ^ MAX_PRECISION)
        This functions assumes that the numerator is larger than or equal to the denominator, because the output would be negative otherwise.
    */
    function ln(uint128 _numerator, uint128 _denominator) internal constant returns (uint128) {
        assert(_numerator <= MAX_NUM);

        uint128 res = 0;
        uint128 x = _numerator * FIXED_1 / _denominator;

        // If x >= 2, then we compute the integer part of log2(x), which is larger than 0.
        if (x >= FIXED_2) {
            uint8 count = floorLog2(x / FIXED_1);
            x >>= count;
            // now x < 2
            res = count * FIXED_1;
        }

        // If x > 1, then we compute the fraction part of log2(x), which is larger than 0.
        if (x > FIXED_1) {
            for (uint8 i = MAX_PRECISION; i > 0; --i) {
                x = (x * x) / FIXED_1;
                // now 1 < x < 4
                if (x >= FIXED_2) {
                    x >>= 1;
                    // now 1 < x < 2
                    res += ONE << (i - 1);
                }
            }
        }

        return (res * LN2_MANTISSA) >> LN2_EXPONENT;
    }

    /**
        Compute the largest integer smaller than or equal to the binary logarithm of the input.
    */
    function floorLog2(uint128 _n) internal constant returns (uint8) {
        uint8 res = 0;

        if (_n < 128) {
            // At most 8 iterations
            while (_n > 1) {
                _n >>= 1;
                res += 1;
            }
        }
        else {
            // Exactly 8 iterations
            for (uint8 s = 64; s > 0; s >>= 1) {
                if (_n >= (ONE << s)) {
                    _n >>= s;
                    res |= s;
                }
            }
        }

        return res;
    }

    /**
        The global "maxExpArray" is sorted in descending order, and therefore the following statements are equivalent:
        - This function finds the position of [the smallest value in "maxExpArray" larger than or equal to "x"]
        - This function finds the highest position of [a value in "maxExpArray" larger than or equal to "x"]
    */
    function findPositionInMaxExpArray(uint128 _x) internal constant returns (uint8) {
        uint8 lo = MIN_PRECISION;
        uint8 hi = MAX_PRECISION;

        while (lo + 1 < hi) {
            uint8 mid = (lo + hi) / 2;
            if (maxExpArray[mid] >= _x)
                lo = mid;
            else
                hi = mid;
        }

        if (maxExpArray[hi] >= _x)
            return hi;
        if (maxExpArray[lo] >= _x)
            return lo;

        assert(false);
        return 0;
    }

    /**
        This function can be auto-generated by the script 'PrintFunctionFixedExp.py'.
        It approximates "e ^ x" via maclaurin summation: "(x^0)/0! + (x^1)/1! + ... + (x^n)/n!".
        It returns "e ^ (x / 2 ^ precision) * 2 ^ precision", that is, the result is upshifted for accuracy.
        The global "maxExpArray" maps each "precision" to "((maximumExponent + 1) << (MAX_PRECISION - precision)) - 1".
        The maximum permitted value for "x" is therefore given by "maxExpArray[precision] >> (MAX_PRECISION - precision)".
    */
    function fixedExp(uint128 _x, uint8 _precision) internal constant returns (uint128) {
        uint128 xi = _x;
        uint128 res = 0;

        xi = (xi * _x) >> _precision;
        res += xi * 0x03442c4e6074a82f1797f72ac0000000;
        // add x^2 * (33! / 2!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0116b96f757c380fb287fd0e40000000;
        // add x^3 * (33! / 3!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0045ae5bdd5f0e03eca1ff4390000000;
        // add x^4 * (33! / 4!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000defabf91302cd95b9ffda50000000;
        // add x^5 * (33! / 5!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0002529ca9832b22439efff9b8000000;
        // add x^6 * (33! / 6!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000054f1cf12bd04e516b6da88000000;
        // add x^7 * (33! / 7!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000a9e39e257a09ca2d6db51000000;
        // add x^8 * (33! / 8!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0000012e066e7b839fa050c309000000;
        // add x^9 * (33! / 9!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0000001e33d7d926c329a1ad1a800000;
        // add x^10 * (33! / 10!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000002bee513bdb4a6b19b5f800000;
        // add x^11 * (33! / 11!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000000003a9316fa79b88eccf2a00000;
        // add x^12 * (33! / 12!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000000048177ebe1fa812375200000;
        // add x^13 * (33! / 13!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000000005263fe90242dcbacf00000;
        // add x^14 * (33! / 14!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0000000000057e22099c030d94100000;
        // add x^15 * (33! / 15!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000000000057e22099c030d9410000;
        // add x^16 * (33! / 16!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000000000000052b6b54569976310000;
        // add x^17 * (33! / 17!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000000000000004985f67696bf748000;
        // add x^18 * (33! / 18!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0000000000000003dea12ea99e498000;
        // add x^19 * (33! / 19!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000000000000000031880f2214b6e000;
        // add x^20 * (33! / 20!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0000000000000000025bcff56eb36000;
        // add x^21 * (33! / 21!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0000000000000000001b722e10ab1000;
        // add x^22 * (33! / 22!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000000000000000001317c70077000;
        // add x^23 * (33! / 23!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000000000000000000000cba84aafa00;
        // add x^24 * (33! / 24!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000000000000000000000082573a0a00;
        // add x^25 * (33! / 25!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000000000000000000000005035ad900;
        // add x^26 * (33! / 26!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x0000000000000000000000002f881b00;
        // add x^27 * (33! / 27!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000000000000000000000001b29340;
        // add x^28 * (33! / 28!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x000000000000000000000000000efc40;
        // add x^29 * (33! / 29!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000000000000000000000000007fe0;
        // add x^30 * (33! / 30!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000000000000000000000000000420;
        // add x^31 * (33! / 31!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000000000000000000000000000021;
        // add x^32 * (33! / 32!)
        xi = (xi * _x) >> _precision;
        res += xi * 0x00000000000000000000000000000001;
        // add x^33 * (33! / 33!)

        return res / 0x688589cc0e9505e2f2fee5580000000 + _x + (ONE << _precision);
        // divide by 33! and then add x^1 / 1! + x^0 / 0!
    }
}


/*
    The BancorGasPriceLimit contract serves as an extra front-running attack mitigation mechanism.
    It sets a maximum gas price on all bancor conversions, which prevents users from "cutting in line"
    in order to front-run other transactions.
    The gas price limit is universal to all converters and it can be updated by the owner to be in line
    with the network's current gas price.
*/
contract BancorGasPriceLimit is IBancorGasPriceLimit, Owned, Utils {
    uint128 public gasPrice = 0 wei;    // maximum gas price for bancor transactions

    /**
        @dev constructor

        @param _gasPrice    gas price limit
    */
    function BancorGasPriceLimit(uint128 _gasPrice)
    greaterThanZero(_gasPrice)
    {
        gasPrice = _gasPrice;
    }

    /*
        @dev allows the owner to update the gas price limit

        @param _gasPrice    new gas price limit
    */
    function setGasPrice(uint128 _gasPrice)
    public
    ownerOnly
    greaterThanZero(_gasPrice)
    {
        gasPrice = _gasPrice;
    }
}


/*
    BancorPriceFloor v0.1

    The bancor price floor contract is a simple contract that allows selling smart tokens for a constant ETH price

    'Owned' is specified here for readability reasons
*/
contract BancorPriceFloor is Owned, TokenHolder {
    uint128 public constant TOKEN_PRICE_N = 1;      // crowdsale price in wei (numerator)
    uint128 public constant TOKEN_PRICE_D = 100;    // crowdsale price in wei (denominator)

    string public version = '0.1';
    ISmartToken public token; // smart token the contract allows selling

    /**
        @dev constructor

        @param _token   smart token the contract allows selling
    */
    function BancorPriceFloor(ISmartToken _token)
    validAddress(_token)
    {
        token = _token;
    }

    /**
        @dev sells the smart token for ETH
        note that the function will sell the full allowance amount

        @return ETH sent in return
    */
    function sell() public returns (uint128 amount) {
        uint128 allowance = token.allowance(msg.sender, this);
        // get the full allowance amount
        assert(token.transferFrom(msg.sender, this, allowance));
        // transfer all tokens from the sender to the contract
        uint128 etherValue = safeMul(allowance, TOKEN_PRICE_N) / TOKEN_PRICE_D;
        // calculate ETH value of the tokens
        msg.sender.transfer(etherValue);
        // send the ETH amount to the seller
        return etherValue;
    }

    /**
        @dev withdraws ETH from the contract

        @param _amount  amount of ETH to withdraw
    */
    function withdraw(uint128 _amount) public ownerOnly {
        msg.sender.transfer(_amount);
        // send the amount
    }

    /**
        @dev deposits ETH in the contract
    */
    function() public payable {
    }
}


/*
    The BancorQuickConverter contract provides allows converting between any token in the
    bancor network in a single transaction.

    A note on conversion paths -
    Conversion path is a data structure that's used when converting a token to another token in the bancor network
    when the conversion cannot necessarily be done by single converter and might require multiple 'hops'.
    The path defines which converters should be used and what kind of conversion should be done in each step.

    The path format doesn't include complex structure and instead, it is represented by a single array
    in which each 'hop' is represented by a 2-tuple - smart token & to token.
    In addition, the first element is always the source token.
    The smart token is only used as a pointer to a converter (since converter addresses are more likely to change).

    Format:
    [source token, smart token, to token, smart token, to token...]
*/
contract BancorQuickConverter is IBancorQuickConverter, TokenHolder {
    mapping(address => bool) public etherTokens;   // list of all supported ether tokens

    /**
        @dev constructor
    */
    function BancorQuickConverter() {
    }

    // validates a conversion path - verifies that the number of elements is odd and that maximum number of 'hops' is 10
    modifier validConversionPath(IERC20Token[] _path) {
        require(_path.length > 2 && _path.length <= (1 + 2 * 10) && _path.length % 2 == 1);
        _;
    }

    /**
        @dev allows the owner to register/unregister ether tokens

        @param _token       ether token contract address
        @param _register    true to register, false to unregister
    */
    function registerEtherToken(IEtherToken _token, bool _register)
    public
    ownerOnly
    validAddress(_token)
    notThis(_token)
    {
        etherTokens[_token] = _register;
    }

    /**
        @dev converts the token to any other token in the bancor network by following
        a predefined conversion path and transfers the result tokens to a target account
        note that the converter should already own the source tokens

        @param _path        conversion path, see conversion path format above
        @param _amount      amount to convert from (in the initial source token)
        @param _minReturn   if the conversion results in an amount smaller than the minimum return - it is cancelled, must be nonzero
        @param _for         account that will receive the conversion result

        @return tokens issued in return
    */
    function convertFor(IERC20Token[] _path, uint128 _amount, uint128 _minReturn, address _for)
    public
    payable
    validConversionPath(_path)
    returns (uint128)
    {
        // if ETH is provided, ensure that the amount is identical to _amount and verify that the source token is an ether token
        IERC20Token fromToken = _path[0];
        require(uint128(msg.value) == 0 || (_amount == uint128(msg.value) && etherTokens[fromToken]));

        // if ETH was sent with the call, the source is an ether token - deposit the ETH in it
        // otherwise, we assume we already have the tokens
        if (uint128(msg.value) > 0)
            IEtherToken(fromToken).deposit.value(uint128(msg.value))();

        // iterate over the conversion path
        IERC20Token toToken = _path[iterateConversionPath(_path, _amount, _minReturn)];

        // finished the conversion, transfer the funds to the target account
        // if the target token is an ether token, withdraw the tokens and send them as ETH
        // otherwise, transfer the tokens as is
        if (etherTokens[toToken])
            IEtherToken(toToken).withdrawTo(_for, _amount);
        else
            assert(toToken.transfer(_for, _amount));

        return _amount;
    }

    function iterateConversionPath(IERC20Token[] _path, uint128 _amount, uint128 _minReturn)
    private
    returns (uint128)
    {
        uint128 to = 2;

        for (; to < _path.length; to += 2) {
            // The following variables have been replaced with expressions to save stack space
            /*
            IERC20Token fromToken = _path[to - 2];
            ISmartToken smartToken = ISmartToken(_path[to - 1]);
            IERC20Token toToken = _path[to];
            */

            ITokenConverter converter = ITokenConverter(ISmartToken(_path[to - 1]).owner());

            // if the smart token isn't the source (from token), the converter doesn't have control over it and thus we need to approve the request
            if (ISmartToken(_path[to - 1]) != _path[to - 2])
                ensureAllowance(_path[to - 2], converter, _amount);

            // make the conversion - if it's the last one, also provide the minimum return value
            _amount = converter.change(_path[to - 2], _path[to], _amount, to == _path.length - 1 ? _minReturn : 1);
        }

        return to;
    }

    /**
        @dev claims the caller's tokens, converts them to any other token in the bancor network
        by following a predefined conversion path and transfers the result tokens to a target account
        note that allowance must be set beforehand

        @param _path        conversion path, see conversion path format above
        @param _amount      amount to convert from (in the initial source token)
        @param _minReturn   if the conversion results in an amount smaller than the minimum return - it is cancelled, must be nonzero
        @param _for         account that will receive the conversion result

        @return tokens issued in return
    */
    function claimAndConvertFor(IERC20Token[] _path, uint128 _amount, uint128 _minReturn, address _for) public returns (uint128) {
        // we need to transfer the tokens from the caller to the converter before we follow
        // the conversion path, to allow it to execute the conversion on behalf of the caller
        // note: we assume we already have allowance
        IERC20Token fromToken = _path[0];
        assert(fromToken.transferFrom(msg.sender, this, _amount));
        return convertFor(_path, _amount, _minReturn, _for);
    }

    /**
        @dev converts the token to any other token in the bancor network by following
        a predefined conversion path and transfers the result tokens back to the sender
        note that the converter should already own the source tokens

        @param _path        conversion path, see conversion path format above
        @param _amount      amount to convert from (in the initial source token)
        @param _minReturn   if the conversion results in an amount smaller than the minimum return - it is cancelled, must be nonzero

        @return tokens issued in return
    */
    function convert(IERC20Token[] _path, uint128 _amount, uint128 _minReturn) public payable returns (uint128) {
        return convertFor(_path, _amount, _minReturn, msg.sender);
    }

    /**
        @dev claims the caller's tokens, converts them to any other token in the bancor network
        by following a predefined conversion path and transfers the result tokens back to the sender
        note that allowance must be set beforehand

        @param _path        conversion path, see conversion path format above
        @param _amount      amount to convert from (in the initial source token)
        @param _minReturn   if the conversion results in an amount smaller than the minimum return - it is cancelled, must be nonzero

        @return tokens issued in return
    */
    function claimAndConvert(IERC20Token[] _path, uint128 _amount, uint128 _minReturn) public returns (uint128) {
        return claimAndConvertFor(_path, _amount, _minReturn, msg.sender);
    }

    /**
        @dev utility, checks whether allowance for the given spender exists and approves one if it doesn't

        @param _token   token to check the allowance in
        @param _spender approved address
        @param _value   allowance amount
    */
    function ensureAllowance(IERC20Token _token, address _spender, uint128 _value) private {
        // check if allowance for the given amount already exists
        if (_token.allowance(this, _spender) >= _value)
            return;

        // if the allowance is nonzero, must reset it to 0 first
        if (_token.allowance(this, _spender) != 0)
            assert(_token.approve(_spender, 0));

        // approve the new allowance
        assert(_token.approve(_spender, _value));
    }
}
/*
    Crowdsale v0.1

    The crowdsale version of the smart token controller, allows contributing ether in exchange for Bancor tokens
    The price remains fixed for the entire duration of the crowdsale
    Note that 20% of the contributions are the BNT token's ETH connector balance
*/
contract CrowdsaleController is SmartTokenController {
    uint128 public constant DURATION = 14 days;                 // crowdsale duration
    uint128 public constant TOKEN_PRICE_N = 1;                  // initial price in wei (numerator)
    uint128 public constant TOKEN_PRICE_D = 100;                // initial price in wei (denominator)
    uint128 public constant BTCS_ETHER_CAP = 50000 ether;       // maximum bitcoin suisse ether contribution
    uint128 public constant MAX_GAS_PRICE = 50000000000 wei;    // maximum gas price for contribution transactions

    string public version = '0.1';

    uint128 public startTime = 0;                   // crowdsale start time (in seconds)
    uint128 public endTime = 0;                     // crowdsale end time (in seconds)
    uint128 public totalEtherCap = 1000000 ether;   // current ether contribution cap, initialized with a temp value as a safety mechanism until the real cap is revealed
    uint128 public totalEtherContributed = 0;       // ether contributed so far
    bytes32 public realEtherCapHash;                // ensures that the real cap is predefined on deployment and cannot be changed later
    address public beneficiary = 0x0;               // address to receive all ether contributions
    address public btcs = 0x0;                      // bitcoin suisse address

    // triggered on each contribution
    event Contribution(address indexed _contributor, uint128 _amount, uint128 _return);

    /**
        @dev constructor

        @param _token          smart token the crowdsale is for
        @param _startTime      crowdsale start time
        @param _beneficiary    address to receive all ether contributions
        @param _btcs           bitcoin suisse address
    */
    function CrowdsaleController(ISmartToken _token, uint128 _startTime, address _beneficiary, address _btcs, bytes32 _realEtherCapHash)
    SmartTokenController(_token)
    validAddress(_beneficiary)
    validAddress(_btcs)
    earlierThan(_startTime)
    greaterThanZero(uint128(_realEtherCapHash))
    {
        startTime = _startTime;
        endTime = startTime + DURATION;
        beneficiary = _beneficiary;
        btcs = _btcs;
        realEtherCapHash = _realEtherCapHash;
    }

    // verifies that the gas price is lower than 50 gwei
    modifier validGasPrice() {
        assert(tx.gasprice <= MAX_GAS_PRICE);
        _;
    }

    // verifies that the ether cap is valid based on the key provided
    modifier validEtherCap(uint128 _cap, uint128 _key) {
        require(computeRealCap(_cap, _key) == realEtherCapHash);
        _;
    }

    // ensures that it's earlier than the given time
    modifier earlierThan(uint128 _time) {
        assert(now < _time);
        _;
    }

    // ensures that the current time is between _startTime (inclusive) and _endTime (exclusive)
    modifier between(uint128 _startTime, uint128 _endTime) {
        assert(now >= _startTime && now < _endTime);
        _;
    }

    // ensures that the sender is bitcoin suisse
    modifier btcsOnly() {
        assert(msg.sender == btcs);
        _;
    }

    // ensures that we didn't reach the ether cap
    modifier etherCapNotReached(uint128 _contribution) {
        assert(safeAdd(totalEtherContributed, _contribution) <= totalEtherCap);
        _;
    }

    // ensures that we didn't reach the bitcoin suisse ether cap
    modifier btcsEtherCapNotReached(uint128 _ethContribution) {
        assert(safeAdd(totalEtherContributed, _ethContribution) <= BTCS_ETHER_CAP);
        _;
    }

    /**
        @dev computes the real cap based on the given cap & key

        @param _cap    cap
        @param _key    key used to compute the cap hash

        @return computed real cap hash
    */
    function computeRealCap(uint128 _cap, uint128 _key) public constant returns (bytes32) {
        return keccak256(_cap, _key);
    }

    /**
        @dev enables the real cap defined on deployment

        @param _cap    predefined cap
        @param _key    key used to compute the cap hash
    */
    function enableRealCap(uint128 _cap, uint128 _key)
    public
    ownerOnly
    active
    between(startTime, endTime)
    validEtherCap(_cap, _key)
    {
        require(_cap < totalEtherCap);
        // validate input
        totalEtherCap = _cap;
    }

    /**
        @dev computes the number of tokens that should be issued for a given contribution

        @param _contribution    contribution amount

        @return computed number of tokens
    */
    function computeReturn(uint128 _contribution) public constant returns (uint128) {
        return safeMul(_contribution, TOKEN_PRICE_D) / TOKEN_PRICE_N;
    }

    /**
        @dev ETH contribution
        can only be called during the crowdsale

        @return tokens issued in return
    */
    function contributeETH()
    public
    payable
    between(startTime, endTime)
    returns (uint128 amount)
    {
        return processContribution();
    }

    /**
        @dev Contribution through BTCs (Bitcoin Suisse only)
        can only be called before the crowdsale started

        @return tokens issued in return
    */
    function contributeBTCs()
    public
    payable
    btcsOnly
    btcsEtherCapNotReached(uint128(msg.value))
    earlierThan(startTime)
    returns (uint128 amount)
    {
        return processContribution();
    }

    /**
        @dev handles contribution logic
        note that the Contribution event is triggered using the sender as the contributor, regardless of the actual contributor

        @return tokens issued in return
    */
    function processContribution() private
    active
    etherCapNotReached(uint128(msg.value))
    validGasPrice
    returns (uint128 amount)
    {
        uint128 tokenAmount = computeReturn(uint128(msg.value));
        beneficiary.transfer(uint128(msg.value));
        // transfer the ether to the beneficiary account
        totalEtherContributed = safeAdd(totalEtherContributed, uint128(msg.value));
        // update the total contribution amount
        token.issue(msg.sender, tokenAmount);
        // issue new funds to the contributor in the smart token
        token.issue(beneficiary, tokenAmount);
        // issue tokens to the beneficiary

        Contribution(msg.sender, uint128(msg.value), tokenAmount);
        return tokenAmount;
    }

    // fallback
    function() payable {
        contributeETH();
    }
}







/**
    ERC20 Standard Token implementation
*/
contract ERC20Token is IERC20Token, Utils {
    string public standard = 'Token 0.1';
    string public name = '';
    string public symbol = '';
    uint8 public decimals = 0;
    uint128 public totalSupply = 0;
    mapping(address => uint128) public balanceOf;
    mapping(address => mapping(address => uint128)) public allowance;

    event Transfer(address indexed _from, address indexed _to, uint128 _value);
    event Approval(address indexed _owner, address indexed _spender, uint128 _value);

    /**
        @dev constructor

        @param _name        token name
        @param _symbol      token symbol
        @param _decimals    decimal points, for display purposes
    */
    function ERC20Token(string _name, string _symbol, uint8 _decimals) {
        require(bytes(_name).length > 0 && bytes(_symbol).length > 0);
        // validate input

        name = _name;
        symbol = _symbol;
        decimals = _decimals;
    }

    /**
        @dev send coins
        throws on any error rather then return a false flag to minimize user errors

        @param _to      target address
        @param _value   transfer amount

        @return true if the transfer was successful, false if it wasn't
    */
    function transfer(address _to, uint128 _value)
    public
    validAddress(_to)
    returns (bool success)
    {
        balanceOf[msg.sender] = safeSub(balanceOf[msg.sender], _value);
        balanceOf[_to] = safeAdd(balanceOf[_to], _value);
        Transfer(msg.sender, _to, _value);
        return true;
    }

    /**
        @dev an account/contract attempts to get the coins
        throws on any error rather then return a false flag to minimize user errors

        @param _from    source address
        @param _to      target address
        @param _value   transfer amount

        @return true if the transfer was successful, false if it wasn't
    */
    function transferFrom(address _from, address _to, uint128 _value)
    public
    validAddress(_from)
    validAddress(_to)
    returns (bool success)
    {
        allowance[_from][msg.sender] = safeSub(allowance[_from][msg.sender], _value);
        balanceOf[_from] = safeSub(balanceOf[_from], _value);
        balanceOf[_to] = safeAdd(balanceOf[_to], _value);
        Transfer(_from, _to, _value);
        return true;
    }

    /**
        @dev allow another account/contract to spend some tokens on your behalf
        throws on any error rather then return a false flag to minimize user errors

        also, to minimize the risk of the approve/transferFrom attack vector
        (see https://docs.google.com/document/d/1YLPtQxZu1UAvO9cZ1O2RPXBbT0mooh4DYKjA_jp-RLM/), approve has to be called twice
        in 2 separate transactions - once to change the allowance to 0 and secondly to change it to the new allowance value

        @param _spender approved address
        @param _value   allowance amount

        @return true if the approval was successful, false if it wasn't
    */
    function approve(address _spender, uint128 _value)
    public
    validAddress(_spender)
    returns (bool success)
    {
        // if the allowance isn't 0, it can only be updated to 0 to prevent an allowance change immediately after withdrawal
        require(_value == 0 || allowance[msg.sender][_spender] == 0);

        allowance[msg.sender][_spender] = _value;
        Approval(msg.sender, _spender, _value);
        return true;
    }
}

/*
    Ether Token interface
*/
contract IEtherToken is ITokenHolder, IERC20Token {
    function deposit() public payable;

    function withdraw(uint128 _amount) public;

    function withdrawTo(address _to, uint128 _amount);
}


/**
    Ether tokenization contract

    'Owned' is specified here for readability reasons
*/
contract EtherToken is IEtherToken, Owned, ERC20Token, TokenHolder {
    // triggered when the total supply is increased
    event Issuance(uint128 _amount);
    // triggered when the total supply is decreased
    event Destruction(uint128 _amount);

    /**
        @dev constructor
    */
    function EtherToken()
    ERC20Token('Ether Token', 'ETH', 8) {
    }

    /**
        @dev deposit ether in the account
    */
    function deposit() public payable {
        balanceOf[msg.sender] = safeAdd(balanceOf[msg.sender], uint128(msg.value));
        // add the value to the account balance
        totalSupply = safeAdd(totalSupply, uint128(msg.value));
        // increase the total supply

        Issuance(uint128(msg.value));
        Transfer(this, msg.sender, uint128(msg.value));
    }

    /**
        @dev withdraw ether from the account

        @param _amount  amount of ether to withdraw
    */
    function withdraw(uint128 _amount) public {
        withdrawTo(msg.sender, _amount);
    }

    /**
        @dev withdraw ether from the account to a target account

        @param _to      account to receive the ether
        @param _amount  amount of ether to withdraw
    */
    function withdrawTo(address _to, uint128 _amount)
    public
    notThis(_to)
    {
        balanceOf[msg.sender] = safeSub(balanceOf[msg.sender], _amount);
        // deduct the amount from the account balance
        totalSupply = safeSub(totalSupply, _amount);
        // decrease the total supply
        _to.transfer(_amount);
        // send the amount to the target account

        Transfer(msg.sender, this, _amount);
        Destruction(_amount);
    }

    // ERC20 standard method overrides with some extra protection

    /**
        @dev send coins
        throws on any error rather then return a false flag to minimize user errors

        @param _to      target address
        @param _value   transfer amount

        @return true if the transfer was successful, false if it wasn't
    */
    function transfer(address _to, uint128 _value)
    public
    notThis(_to)
    returns (bool success)
    {
        assert(super.transfer(_to, _value));
        return true;
    }

    /**
        @dev an account/contract attempts to get the coins
        throws on any error rather then return a false flag to minimize user errors

        @param _from    source address
        @param _to      target address
        @param _value   transfer amount

        @return true if the transfer was successful, false if it wasn't
    */
    function transferFrom(address _from, address _to, uint128 _value)
    public
    notThis(_to)
    returns (bool success)
    {
        assert(super.transferFrom(_from, _to, _value));
        return true;
    }

    /**
        @dev deposit ether in the account
    */
    function() public payable {
        deposit();
    }
}

contract TestERC20Token is ERC20Token {
    function TestERC20Token(string _name, string _symbol, uint128 _supply)
    public
    ERC20Token(_name, _symbol, 0)
    {
        totalSupply = _supply;
        balanceOf[msg.sender] = _supply;
    }
}

/*
    Smart Token v0.3

    'Owned' is specified here for readability reasons
*/
contract SmartToken is ISmartToken, Owned, ERC20Token, TokenHolder {
    string public version = '0.3';

    bool public transfersEnabled = true;    // true if transfer/transferFrom are enabled, false if not

    // triggered when a smart token is deployed - the _token address is defined for forward compatibility, in case we want to trigger the event from a factory
    event NewSmartToken(address _token);
    // triggered when the total supply is increased
    event Issuance(uint128 _amount);
    // triggered when the total supply is decreased
    event Destruction(uint128 _amount);

    /**
        @dev constructor

        @param _name       token name
        @param _symbol     token short symbol, minimum 1 character
        @param _decimals   for display purposes only
    */
    function SmartToken(string _name, string _symbol, uint8 _decimals)
    ERC20Token(_name, _symbol, _decimals)
    {
        NewSmartToken(address(this));
    }

    // allows execution only when transfers aren't disabled
    modifier transfersAllowed {
        assert(transfersEnabled);
        _;
    }

    /**
        @dev disables/enables transfers
        can only be called by the contract owner

        @param _disable    true to disable transfers, false to enable them
    */
    function disableTransfers(bool _disable) public ownerOnly {
        transfersEnabled = !_disable;
    }

    /**
        @dev increases the token supply and sends the new tokens to an account
        can only be called by the contract owner

        @param _to         account to receive the new amount
        @param _amount     amount to increase the supply by
    */
    function issue(address _to, uint128 _amount)
    public
    //ownerOnly
    validAddress(_to)
    notThis(_to)
    {
        totalSupply = safeAdd(totalSupply, _amount);
        balanceOf[_to] = safeAdd(balanceOf[_to], _amount);

        Issuance(_amount);
        Transfer(this, _to, _amount);
    }

    /**
        @dev removes tokens from an account and decreases the token supply
        can be called by the contract owner to destroy tokens from any account or by any holder to destroy tokens from his/her own account

        @param _from       account to remove the amount from
        @param _amount     amount to decrease the supply by
    */
    function destroy(address _from, uint128 _amount) public {
        //require(msg.sender == _from || msg.sender == owner);
        // validate input

        balanceOf[_from] = safeSub(balanceOf[_from], _amount);
        totalSupply = safeSub(totalSupply, _amount);

        Transfer(_from, this, _amount);
        Destruction(_amount);
    }

    // ERC20 standard method overrides with some extra functionality

    /**
        @dev send coins
        throws on any error rather then return a false flag to minimize user errors
        in addition to the standard checks, the function throws if transfers are disabled

        @param _to      target address
        @param _value   transfer amount

        @return true if the transfer was successful, false if it wasn't
    */
    function transfer(address _to, uint128 _value) public transfersAllowed returns (bool success) {
        assert(super.transfer(_to, _value));
        return true;
    }

    /**
        @dev an account/contract attempts to get the coins
        throws on any error rather then return a false flag to minimize user errors
        in addition to the standard checks, the function throws if transfers are disabled

        @param _from    source address
        @param _to      target address
        @param _value   transfer amount

        @return true if the transfer was successful, false if it wasn't
    */
    function transferFrom(address _from, address _to, uint128 _value) public transfersAllowed returns (bool success) {
        assert(super.transferFrom(_from, _to, _value));
        return true;
    }
}



/*
    We consider every contract to be a 'token holder' since it's currently not possible
    for a contract to deny receiving tokens.

    The TokenHolder's contract sole purpose is to provide a safety mechanism that allows
    the owner to send tokens that were sent to the contract by mistake back to their sender.
*/
