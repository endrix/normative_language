class Model:
    def halt(self):
        self.halted = True

    def stay(self):
        return self.halted

    def reset(self):
        self.halted = False
