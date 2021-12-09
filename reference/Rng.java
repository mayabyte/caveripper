import java.util.ArrayList;
import java.util.List;

public class Rng {
    static int seed;
    public static void main(String args[]) {
        seed = 0x12345678;
        for (int i = 0; i < 2000; i++) {
            List<Integer> l = new ArrayList<Integer>();
            for (int j = 0; j < i; j++) {
                l.add(j);
            }
            randSwaps(l);
            for (int x:l) {
                System.out.print(x + ",");
            }
            System.out.println();
        }
    }

    static int rand() { // the star of the show!
        seed = seed * 0x41c64e6d + 0x3039;
        int ret = (seed >>> 0x10) & 0x7fff;
        return ret;
    }
    static int randInt(int max) {
        return (int)(rand() * max / 32768.0f);
    }
    static float randFloat() {
        return rand() / 32768.0f;
    }
    static int randIndexWeight(List<Integer> weights) {
        int sum = 0;
        for (int w: weights) sum += w;
        int cumSum = 0;
        int r = randInt(sum);
        for (int i = 0; i < weights.size(); i++) {
            cumSum += weights.get(i);
            if (cumSum > r)
                return i;
        }
        return -1;
     }
    static <T> void randBacks(List<T> a) {
        for (int i = 0; i < a.size(); i++) {
            int r = randInt(a.size());
            a.add(a.remove(r));
        }
    }
    static <T> void randBacks(List<T> a, int num) {
        for (int i = 0; i < num; i++) {
            int r = randInt(a.size());
            a.add(a.remove(r));
        }
    }
    static <T> void randSwaps(List<T> a) {
        for (int i = 0; i < a.size(); i++) {
            int r = randInt(a.size());
            T temp = a.get(i);
            a.set(i, a.get(r));
            a.set(r, temp);
        }
    }
}
