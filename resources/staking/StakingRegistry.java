package Staking;

import avm.Address;
import avm.Blockchain;
import avm.Result;

import org.aion.avm.userlib.AionMap;

import java.math.BigInteger;
import java.util.Map;

public class StakingRegistry {
    
    static {
        stakers = new AionMap();
    }
    
    private static class Staker {
        private BigInteger totalVote;
        
        // maps addresses to the votes those addresses have sent to this staker
        // the sum of votes.values() should always equal totalVote
        private Map<Address, BigInteger> votes;
    }
    
    private static Map<Address, Staker> stakers;

    private static byte[] seed;
    
    public static byte[] main() {
        return null;
    }

    public static boolean register(Address address) {
        if (Blockchain.getCaller().equals(address)) {
            stakers.put(address, new Staker());
            return true;
        } else {
            return false;
        }
    }

    public static void vote(Address stakerAddress) {
        BigInteger value = Blockchain.getValue();
        Address senderAddress = Blockchain.getCaller();
        if (null != stakerAddress && stakers.containsKey(stakerAddress) && value.compareTo(BigInteger.ZERO) > 0) {
            Staker staker = stakers.get(stakerAddress);
            staker.totalVote = staker.totalVote.add(value);
            
            BigInteger vote = staker.votes.get(senderAddress);
            if (null == vote) {
                // This is the first time the sender has voted for this staker
                staker.votes.put(senderAddress, value);
            } else {
                staker.votes.replace(senderAddress, vote.add(value));
            }
        }
    }

    public static void unvote(Address stakerAddress, long amount) {
        Address senderAddress = Blockchain.getCaller();
        Blockchain.require(amount >= 0);
        BigInteger amountBI = BigInteger.valueOf(amount);
        if (null != stakerAddress && stakers.containsKey(stakerAddress)) {
            Result result;
            Staker staker = stakers.get(stakerAddress);
            if (staker != null && staker.votes.containsKey(senderAddress)) {
                BigInteger vote = staker.votes.get(senderAddress);
                if (vote.compareTo(amountBI) > 0) {
                    staker.votes.replace(senderAddress, vote.subtract(amountBI));
                    staker.totalVote = staker.totalVote.subtract(amountBI);
                    result = Blockchain.call(senderAddress, amountBI, new byte[0], Blockchain.getRemainingEnergy());
                } else {
                    staker.totalVote = staker.totalVote.subtract(vote);
                    result = Blockchain.call(senderAddress, vote, new byte[0], Blockchain.getRemainingEnergy());
                    staker.votes.remove(senderAddress);
                }              
            }
        }
    }

    public static long getVote(Address stakingAddress) {
        Staker staker = stakers.get(stakingAddress);
        if (staker != null) {
            return staker.totalVote.longValue();
        } else {
            return 0;
        }
    }

    public static byte[] getSeed() {
        return seed;
    }

    public static void setSeed(byte[] newSeed) {
        Blockchain.require(newSeed.length == 32);
        seed = newSeed;
    }
}
