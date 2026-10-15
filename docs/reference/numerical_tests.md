To numerically test if your erosion results and river systems are realistic, you can use **Geomorphometry**, the quantitative analysis of land surfaces. By comparing your procedural output against established hydrological laws and empirical data, you can identify which parameters (like erosion strength or sediment capacity) need adjustment.

Here are 10 quantitative tests you can measure:

### **1\. Bifurcation Ratio ($R\_b$)**

According to **Horton’s Law of Stream Numbers**, the number of streams of a given order decreases in a geometric progression as the order increases.

* **The Test:** Calculate $R\_b \= \\frac{N\_u}{N\_{u+1}}$, where $N\_u$ is the number of streams of order $u$.  
* **Realistic Target:** Natural networks typically have an $R\_b$ between **3.0 and 5.0**. If your ratio is too high, your rivers aren't branching enough.

### **2\. Drainage Density ($D\_d$)**

This measures how "densely" the terrain is packed with stream channels.

* **The Test:** $D\_d \= \\frac{\\sum L}{A}$, where $\\sum L$ is the total length of all channels and $A$ is the total area.  
* **Realistic Target:** Low $D\_d$ indicates resistant or permeable soil; high $D\_d$ indicates rapid runoff and easily erodible terrain. Use this to tweak your **Hardness** or **Rainfall** parameters.

### **3\. Hack’s Law (Length-Area Relationship)**

This empirical relationship describes the elongation of a drainage basin.

* **The Test:** $L \\propto A^h$, where $L$ is the length of the longest stream and $A$ is the basin area.  
* **Realistic Target:** The exponent $h$ typically falls between **0.5 and 0.6**. If $h \< 0.5$, your basins are too "round" or circular.

### **4\. Longitudinal Profile Concavity**

Natural rivers generally have a concave-upward longitudinal profile (steeper at the headwaters, flatter at the mouth).

* **The Test:** Plot elevation vs. distance along your longest river.  
* **Realistic Target:** A smooth, downward-curving line. "Knickpoints" (sharp spikes) indicate where your erosion isn't balancing tectonic uplift or where you have local minima traps.

### **5\. Slope-Area Relationship (Flint's Law)**

In steady-state landscapes, the local slope ($S$) is related to the upstream drainage area ($A$).

* **The Test:** $S \= k\_s A^{-\\theta}$.  
* **Realistic Target:** The concavity index $\\theta$ is usually **0.4 to 0.7**. If your slopes don't decrease as the area increases, your rivers will look like "gutters" rather than natural valleys.

### **6\. Fractal Dimension ($D$)**

River networks are naturally fractal and practically "space-filling".

* **The Test:** Use a **Box-Counting** algorithm on your river mask to find $D$.  
* **Realistic Target:** For the whole network, $D$ should approach **2.0**. For individual stream segments, $D$ is usually between **1.1 and 1.2**.

### **7\. Stream Length Ratio ($R\_L$)**

Based on **Horton’s Law of Stream Lengths**, the average length of streams increases with order.

* **The Test:** $R\_L \= \\frac{\\bar{L}\_{u+1}}{\\bar{L}\_u}$.  
* **Realistic Target:** In nature, this ratio is fairly constant within a basin. If higher-order rivers are too short, your water is likely "evaporating" or getting stuck before reaching the coast.

### **8\. Sinuosity Index**

This measures how much a river meanders.

* **The Test:** $SI \= \\frac{\\text{Actual Length}}{\\text{Straight-line Distance}}$.  
* **Realistic Target:** A value of **1.0** is straight; **\>1.5** is considered meandering. High sinuosity requires a balance of bank erosion and sediment deposition.

### **9\. Drainage Texture**

This describes the relative spacing of drainage lines.

* **The Test:** $T \= \\frac{N\_1}{P}$, where $N\_1$ is the number of first-order streams and $P$ is the perimeter of the basin.  
* **Realistic Target:** This helps you differentiate between "fine" textures (many small streams) and "coarse" textures (few large streams), useful for tweaking **Erosion Strength**.

### **10\. Pit/Sink Count**

While natural lakes exist, a realistic procedural map should not have thousands of tiny, unconnected "potholes."

* **The Test:** Count the number of local minima (pixels with no lower neighbors) that are not at the "sea level" or map edge.  
* **Realistic Target:** As close to **zero** as possible before adding intentional lakes. High counts indicate your **Deposition** or **Pit-Filling** logic is failing.

